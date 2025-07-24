use crate::models::models::{AudioBook, CreateFileMetadata};
use anyhow::Error;
use sqlx::{Pool, Sqlite};

pub async fn insert_audiobook(db: &Pool<Sqlite>, book: &AudioBook) -> Result<(), Error> {
    sqlx::query!(
        r#"
        INSERT INTO audiobooks 
            (author, series, title, files_location, cover_art, metadata)
        VALUES 
            (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
        book.author,
        book.series,
        book.title,
        book.content_path,
        book.cover_art,
        book.metadata,
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn insert_file_metadata(
    db: &Pool<Sqlite>,
    create_data: CreateFileMetadata,
) -> Result<(), Error> {
    sqlx::query!(
        r#"
        INSERT INTO files (book_id, file_path, codec, duration, channels, sample_rate)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
        create_data.book_id,
        create_data.file_path,
        create_data.codec,
        create_data.duration,
        create_data.channels,
        create_data.sample_rate
    )
    .execute(db)
    .await?;

    Ok(())
}
