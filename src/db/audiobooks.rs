use crate::{
    api::api_error::ApiError,
    models::audiobooks::{AudioBookRow, CreateFileMetadata, FileMetadata},
};
use sqlx::{Pool, Sqlite};

pub async fn list_all_books(db: &Pool<Sqlite>) -> Result<Vec<AudioBookRow>, ApiError> {
    let books = sqlx::query_as::<_, AudioBookRow>(
        r#"
        SELECT b.id, b.author, b.series, b.title, b.book_size, b.files_location, b.cover_art,
               b.duration, b.metadata, b.series_id, b.series_sequence, b.asin, b.narrated_by,
               b.user_locked, b.description, b.series_locked, s.name AS series_name
        FROM audiobooks b LEFT JOIN series s ON s.id = b.series_id
        ORDER BY b.author, b.series, b.title
        "#,
    )
    .fetch_all(db)
    .await?;

    Ok(books)
}

pub async fn get_book(db: &Pool<Sqlite>, book_id: i64) -> Result<AudioBookRow, ApiError> {
    let book = sqlx::query_as::<_, AudioBookRow>(
        r#"
        SELECT b.id, b.author, b.series, b.title, b.book_size, b.files_location, b.cover_art,
               b.duration, b.metadata, b.series_id, b.series_sequence, b.asin, b.narrated_by,
               b.user_locked, b.description, b.series_locked, s.name AS series_name
        FROM audiobooks b LEFT JOIN series s ON s.id = b.series_id
        WHERE b.id = ?1
        "#,
    )
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
