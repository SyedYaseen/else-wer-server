use std::path::Path as StdPath;
use sqlx::{Pool, Sqlite};

use crate::{
    api::api_error::ApiError,
    models::libraries::{CreateLibraryDto, Library, UpdateLibraryDto},
};

fn map_unique_violation(e: sqlx::Error) -> ApiError {
    if e.as_database_error()
        .is_some_and(|d| d.is_unique_violation())
    {
        ApiError::BadRequest("A library with that name or path already exists".into())
    } else {
        ApiError::Database(e)
    }
}

/// True if `a`'s path components are a prefix of `b`'s (or equal) — i.e. `b` is `a`
/// itself or lives under it. Component-based, not filesystem-resolved (paths may not
/// exist yet at validation time), so this catches the common typo/nesting cases
/// without requiring the path to be reachable.
fn is_nested_or_equal(a: &str, b: &str) -> bool {
    let a: Vec<_> = StdPath::new(a).components().collect();
    let b: Vec<_> = StdPath::new(b).components().collect();
    b.len() >= a.len() && b[..a.len()] == a[..]
}

fn paths_overlap(a: &str, b: &str) -> bool {
    is_nested_or_equal(a, b) || is_nested_or_equal(b, a)
}

/// Rejects a candidate library path that is the same as, nested under, or an
/// ancestor of any other library's path. Two libraries sharing/overlapping a root
/// would let the same on-disk folder scan into both — resolve_book_id's
/// (files_location, title) conflict target isn't library-scoped (see meta_scan.rs),
/// so an overlap would let one library's scan silently steal the other's books
/// rather than erroring. Prevented here instead, so that gap can never trigger.
async fn validate_no_path_overlap(
    db: &Pool<Sqlite>,
    candidate_path: &str,
    exclude_id: Option<i64>,
) -> Result<(), ApiError> {
    let existing = list_libraries(db).await?;
    for lib in existing {
        if Some(lib.id) == exclude_id {
            continue;
        }
        if paths_overlap(candidate_path, &lib.path) {
            return Err(ApiError::BadRequest(format!(
                "Path overlaps with library '{}' ({})",
                lib.name, lib.path
            )));
        }
    }
    Ok(())
}

const LIBRARY_COLUMNS: &str = "id, name, path, is_default, created_at";

pub async fn list_libraries(db: &Pool<Sqlite>) -> Result<Vec<Library>, ApiError> {
    let sql = format!("SELECT {LIBRARY_COLUMNS} FROM libraries ORDER BY name");
    let rows = sqlx::query_as::<_, Library>(&sql).fetch_all(db).await?;
    Ok(rows)
}

pub async fn get_library(db: &Pool<Sqlite>, id: i64) -> Result<Library, ApiError> {
    let sql = format!("SELECT {LIBRARY_COLUMNS} FROM libraries WHERE id = ?1");
    let row = sqlx::query_as::<_, Library>(&sql)
        .bind(id)
        .fetch_optional(db)
        .await?;

    row.ok_or_else(|| ApiError::NotFound(format!("No library with id {id}")))
}

/// Fallback target when a caller (e.g. upload) doesn't specify a library: the
/// explicitly-flagged default (at most one, enforced by a partial unique index —
/// see migration 0007), or whichever library was created first if none is flagged
/// (shouldn't happen post-migration, but a library set couldn't be legitimately empty
/// past startup either — ensure_default_library always seeds and flags one). Errors
/// only if no libraries exist at all.
pub async fn get_default_library(db: &Pool<Sqlite>) -> Result<Library, ApiError> {
    let sql = format!(
        "SELECT {LIBRARY_COLUMNS} FROM libraries ORDER BY is_default DESC, id ASC LIMIT 1"
    );
    let row = sqlx::query_as::<_, Library>(&sql).fetch_optional(db).await?;

    row.ok_or_else(|| ApiError::Internal("No libraries configured".into()))
}

pub async fn create_library(db: &Pool<Sqlite>, dto: &CreateLibraryDto) -> Result<Library, ApiError> {
    validate_no_path_overlap(db, &dto.path, None).await?;

    let sql = format!(
        "INSERT INTO libraries (name, path) VALUES (?1, ?2) RETURNING {LIBRARY_COLUMNS}"
    );
    sqlx::query_as::<_, Library>(&sql)
        .bind(&dto.name)
        .bind(&dto.path)
        .fetch_one(db)
        .await
        .map_err(map_unique_violation)
}

pub async fn update_library(
    db: &Pool<Sqlite>,
    id: i64,
    dto: &UpdateLibraryDto,
) -> Result<Library, ApiError> {
    let current = get_library(db, id).await?;
    let name = dto.name.as_ref().unwrap_or(&current.name);
    let path = dto.path.as_ref().unwrap_or(&current.path);

    if dto.path.is_some() {
        validate_no_path_overlap(db, path, Some(id)).await?;
    }

    let mut tx = db.begin().await?;

    // The partial unique index only allows one is_default=1 row at a time, so
    // becoming the default requires clearing the previous one first, in the same
    // transaction (never a window with zero or two defaults visible to a reader).
    if dto.is_default == Some(true) {
        sqlx::query("UPDATE libraries SET is_default = 0 WHERE is_default = 1")
            .execute(&mut *tx)
            .await?;
    }

    let sql = format!(
        r#"
        UPDATE libraries SET name = ?1, path = ?2, is_default = COALESCE(?3, is_default)
        WHERE id = ?4
        RETURNING {LIBRARY_COLUMNS}
        "#
    );
    let library = sqlx::query_as::<_, Library>(&sql)
        .bind(name)
        .bind(path)
        .bind(dto.is_default)
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_unique_violation)?;

    tx.commit().await?;
    Ok(library)
}

/// Cascades to `audiobooks.library_id` (ON DELETE CASCADE) — drops catalog rows for
/// books in this library, never touches files on disk (same semantics as delete_book).
pub async fn delete_library(db: &Pool<Sqlite>, id: i64) -> Result<u64, ApiError> {
    // Enforced server-side (not just the admin UI's disabled button): deleting the
    // last library cascades and wipes the entire catalog, and leaves upload/scan
    // with nothing to fall back to until the process restarts and re-seeds one.
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM libraries")
        .fetch_one(db)
        .await?;
    if count <= 1 {
        return Err(ApiError::BadRequest(
            "Can't delete the last remaining library".into(),
        ));
    }

    let mut tx = db.begin().await?;

    let result = sqlx::query("DELETE FROM libraries WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    // Deleting the flagged default leaves none flagged; promote the earliest-created
    // survivor so get_default_library always has an explicit answer, not just its
    // (still-correct, but less intentional) id-ordering fallback.
    sqlx::query(
        r#"
        UPDATE libraries SET is_default = 1
        WHERE id = (SELECT id FROM libraries ORDER BY id ASC LIMIT 1)
          AND NOT EXISTS (SELECT 1 FROM libraries WHERE is_default = 1)
        "#,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(result.rows_affected())
}
