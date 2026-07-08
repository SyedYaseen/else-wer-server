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
               b.user_locked, s.name AS series_name
        FROM audiobooks b LEFT JOIN series s ON s.id = b.series_id
        ORDER BY b.author, b.series, b.title
        "#,
    )
    .fetch_all(db)
    .await?;

    Ok(books)
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

pub async fn get_files_by_book_id(
    db: &Pool<Sqlite>,
    book_id: i64,
) -> Result<Vec<FileMetadata>, ApiError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            id,
            book_id,
            file_id,
            file_name,
            file_size,
            file_path,
            duration,
            channels,
            sample_rate,
            bitrate
        FROM files
        WHERE book_id = ?
        ORDER BY id
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
                    file_id: Some(r.file_id),
                    file_name: r.file_name,
                    file_size: r.file_size,
                    file_path: r.file_path,
                    duration: r.duration,
                    channels: r.channels,
                    sample_rate: r.sample_rate,
                    bitrate: r.bitrate,
                },
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;

    Ok(files)
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
