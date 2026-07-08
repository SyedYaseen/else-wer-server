use crate::api::api_error::ApiError;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::path::Path;
use tokio::fs;

pub const META_FILE_NAME: &str = "metadata.json";

/// Portable per-folder book metadata (Audiobookshelf-style). Written on user edits
/// and match-applies; read back on scan with priority over tag-derived values, so a
/// library survives DB loss and moves between installs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookMetaFile {
    pub version: u32,
    pub title: String,
    pub author: String,
    /// Album/display value (the audiobooks.series column).
    #[serde(default)]
    pub series: Option<String>,
    #[serde(default)]
    pub narrated_by: Option<String>,
    /// Real-world series (saga), from an applied external match.
    #[serde(default)]
    pub series_name: Option<String>,
    #[serde(default)]
    pub series_sequence: Option<String>,
    #[serde(default)]
    pub asin: Option<String>,
    #[serde(default)]
    pub user_locked: bool,
}

#[derive(FromRow)]
struct BookMetaRow {
    title: String,
    author: String,
    series: Option<String>,
    narrated_by: Option<String>,
    series_sequence: Option<String>,
    asin: Option<String>,
    user_locked: bool,
    files_location: String,
    series_name: Option<String>,
}

/// Write {files_location}/metadata.json for a book. Skipped when the folder holds
/// more than one book (loose files — the file would be ambiguous) and tolerant of FS
/// failures: the DB stays the source of truth, the file is a portable copy.
pub async fn write_book_meta_json(pool: &SqlitePool, book_id: i64) -> Result<(), ApiError> {
    let Some(row) = sqlx::query_as::<_, BookMetaRow>(
        r#"
        SELECT b.title, b.author, b.series, b.narrated_by, b.series_sequence, b.asin,
               b.user_locked, b.files_location, s.name AS series_name
        FROM audiobooks b LEFT JOIN series s ON s.id = b.series_id
        WHERE b.id = ?
        "#,
    )
    .bind(book_id)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(());
    };

    let siblings: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audiobooks WHERE files_location = ?")
        .bind(&row.files_location)
        .fetch_one(pool)
        .await?;
    if siblings != 1 {
        tracing::warn!(
            "Skipping {META_FILE_NAME} for book {book_id}: {} books share {}",
            siblings,
            row.files_location
        );
        return Ok(());
    }

    let meta = BookMetaFile {
        version: 1,
        title: row.title,
        author: row.author,
        series: row.series,
        narrated_by: row.narrated_by,
        series_name: row.series_name,
        series_sequence: row.series_sequence,
        asin: row.asin,
        user_locked: row.user_locked,
    };

    let path = Path::new(&row.files_location).join(META_FILE_NAME);
    let json = serde_json::to_string_pretty(&meta)?;
    if let Err(e) = fs::write(&path, json).await {
        tracing::warn!("Failed writing {}: {e}", path.display());
    }

    Ok(())
}

/// Read a folder's metadata.json; None on any failure (missing, unreadable, invalid).
pub async fn read_book_meta_json(path_parent: &str) -> Option<BookMetaFile> {
    let path = Path::new(path_parent).join(META_FILE_NAME);
    let content = fs::read_to_string(&path).await.ok()?;
    match serde_json::from_str(&content) {
        Ok(meta) => Some(meta),
        Err(e) => {
            tracing::warn!("Ignoring invalid {}: {e}", path.display());
            None
        }
    }
}
