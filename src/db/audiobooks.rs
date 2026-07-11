use crate::{
    api::api_error::ApiError,
    models::audiobooks::{AudioBookRow, CreateFileMetadata, FileMetadata},
};
use sqlx::{Pool, QueryBuilder, Sqlite};

// Shared by list_books/get_book so the column list and joins live in exactly one
// place — previously duplicated near-verbatim across four functions.
const BOOK_COLUMNS: &str = r#"
    b.id, b.author, b.series, b.title, b.book_size, b.files_location, b.cover_art,
    b.duration, b.metadata, b.series_id, b.series_sequence, b.asin, b.narrated_by,
    b.user_locked, b.description, b.series_locked, s.name AS series_name,
    b.library_id, l.name AS library_name
"#;

const BOOK_FROM: &str = r#"
    FROM audiobooks b
    LEFT JOIN series s ON s.id = b.series_id
    LEFT JOIN libraries l ON l.id = b.library_id
"#;

/// Unfiltered when both args are None (list_all_books' old behavior). `q` matches
/// title/author/series/narrated_by, case-insensitive substring (same field set as
/// the now-superseded client-side filter in useBookSearch.ts). `library_id` scopes
/// to one library.
pub async fn list_books(
    db: &Pool<Sqlite>,
    q: Option<&str>,
    library_id: Option<i64>,
) -> Result<Vec<AudioBookRow>, ApiError> {
    let pattern = q.map(|q| {
        let escaped = q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
        format!("%{escaped}%")
    });

    let sql = format!(
        r#"
        SELECT {BOOK_COLUMNS}
        {BOOK_FROM}
        WHERE (?1 IS NULL OR b.title LIKE ?1 ESCAPE '\' OR b.author LIKE ?1 ESCAPE '\'
               OR b.series LIKE ?1 ESCAPE '\' OR b.narrated_by LIKE ?1 ESCAPE '\')
          AND (?2 IS NULL OR b.library_id = ?2)
        ORDER BY b.author, b.series, b.title
        "#
    );

    let books = sqlx::query_as::<_, AudioBookRow>(&sql)
        .bind(pattern)
        .bind(library_id)
        .fetch_all(db)
        .await?;

    Ok(books)
}

pub async fn get_book(db: &Pool<Sqlite>, book_id: i64) -> Result<AudioBookRow, ApiError> {
    let sql = format!(
        r#"
        SELECT {BOOK_COLUMNS}
        {BOOK_FROM}
        WHERE b.id = ?1
        "#
    );

    let book = sqlx::query_as::<_, AudioBookRow>(&sql)
        .bind(book_id)
        .fetch_optional(db)
        .await?;

    book.ok_or_else(|| ApiError::NotFound(format!("No book with id {book_id}")))
}

pub async fn update_cover_art(
    db: &Pool<Sqlite>,
    book_id: i64,
    cover_link: String,
) -> Result<(), ApiError> {
    sqlx::query!(
        r#"
        UPDATE audiobooks
        SET cover_art = ?1
        WHERE id = ?2
        "#,
        cover_link,
        book_id
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn update_description(
    db: &Pool<Sqlite>,
    book_id: i64,
    description: &str,
) -> Result<(), ApiError> {
    sqlx::query!(
        r#"
        UPDATE audiobooks
        SET description = ?1
        WHERE id = ?2
        "#,
        description,
        book_id
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn get_files_by_book_id(
    db: &Pool<Sqlite>,
    book_id: i64,
) -> Result<Vec<FileMetadata>, ApiError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            id,
            book_id,
            file_name,
            file_size,
            file_path,
            duration,
            channels,
            sample_rate,
            bitrate,
            track_number,
            disc_number
        FROM files
        WHERE book_id = ?
        ORDER BY COALESCE(disc_number, 0), COALESCE(track_number, 999999), file_name
        "#,
        book_id
    )
    .fetch_all(db)
    .await?;

    let files = rows
        .into_iter()
        .map(|r| {
            Ok(FileMetadata {
                id: r
                    .id
                    .ok_or_else(|| ApiError::Internal("file row missing id".into()))?,
                data: CreateFileMetadata {
                    book_id: r.book_id,
                    // Single id space since 0008: file_id mirrors the row's own id.
                    file_id: r.id,
                    file_name: r.file_name,
                    file_size: Some(r.file_size),
                    file_path: r.file_path,
                    duration: r.duration,
                    channels: r.channels,
                    sample_rate: r.sample_rate,
                    bitrate: r.bitrate,
                    track_number: r.track_number,
                    disc_number: r.disc_number,
                },
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;

    Ok(files)
}

// Manual chapter-order fix: flattens disc_number to 0 and assigns sequential
// track_number per file_ids' position. Safe against rescans since new-file
// scanning is INSERT OR IGNORE and no other code path updates these columns
// on existing rows.
pub async fn reorder_files(
    db: &Pool<Sqlite>,
    book_id: i64,
    file_ids: &[i64],
) -> Result<(), ApiError> {
    let existing: Vec<i64> = sqlx::query_scalar!("SELECT id FROM files WHERE book_id = ?", book_id)
        .fetch_all(db)
        .await?
        .into_iter()
        .flatten()
        .collect();

    let mut existing_sorted = existing.clone();
    existing_sorted.sort_unstable();
    let mut given_sorted = file_ids.to_vec();
    given_sorted.sort_unstable();
    if existing_sorted != given_sorted {
        return Err(ApiError::BadRequest(
            "file_ids must match the book's existing files exactly".into(),
        ));
    }

    let mut qb: QueryBuilder<'_, Sqlite> =
        QueryBuilder::new("UPDATE files SET disc_number = 0, track_number = CASE id");
    for (idx, file_id) in file_ids.iter().enumerate() {
        let track_number = (idx + 1) as i64;
        qb.push(" WHEN ")
            .push_bind(*file_id)
            .push(" THEN ")
            .push_bind(track_number);
    }
    qb.push(" END WHERE book_id = ").push_bind(book_id);
    qb.push(" AND id IN (");
    let mut separated = qb.separated(", ");
    for file_id in file_ids {
        separated.push_bind(*file_id);
    }
    qb.push(")");
    qb.build().execute(db).await?;

    Ok(())
}

// Cascades to `files` and `progress` rows (ON DELETE CASCADE); does not touch
// disk — callers must remove the on-disk files first, since files_location can
// be shared by sibling books.
pub async fn delete_book(db: &Pool<Sqlite>, book_id: i64) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM audiobooks WHERE id = ?")
        .bind(book_id)
        .execute(db)
        .await?;

    Ok(())
}

pub async fn get_file_path_by_id(db: &Pool<Sqlite>, id: i64) -> Result<String, ApiError> {
    let path: (String,) = sqlx::query_as(
        r#"
        SELECT
            file_path
        FROM files
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(db)
    .await?;

    Ok(path.0)
}
