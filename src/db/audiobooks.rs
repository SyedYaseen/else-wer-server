use crate::models::models::{AudioBook, CreateFileMetadata};
use anyhow::{Error, Result};
use sqlx::{Pool, Sqlite};

pub async fn insert_audiobook(db: &Pool<Sqlite>, book: &AudioBook) -> Result<i64, Error> {
    let id = sqlx::query_scalar!(
        r#"
        INSERT INTO audiobooks 
            (author, series, title, files_location, cover_art, metadata)
        VALUES 
            (?1, ?2, ?3, ?4, ?5, ?6)
        RETURNING id
        "#,
        book.author,
        book.series,
        book.title,
        book.content_path,
        book.cover_art,
        book.metadata,
    )
    .fetch_one(db)
    .await?;

    Ok(id)
}

pub async fn insert_file_metadata(
    db: &Pool<Sqlite>,
    create_data: CreateFileMetadata,
) -> Result<(), Error> {
    let file_path = create_data.file_path.to_string().to_owned();
    sqlx::query!(
        r#"
        INSERT INTO files (book_id, file_path, duration, channels, sample_rate, bitrate)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
        create_data.book_id,
        file_path,
        create_data.duration,
        create_data.channels,
        create_data.sample_rate,
        create_data.bitrate
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn get_audiobook_id(db: &Pool<Sqlite>, book: &AudioBook) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(
        r#"
        SELECT id
        FROM audiobooks
        WHERE author = ?1 AND title = ?2
        "#,
    )
    .bind(&book.author)
    .bind(&book.title)
    .fetch_one(db)
    .await?;

    Ok(row.0)
}
