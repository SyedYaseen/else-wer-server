use crate::models::models::AudioBook;
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
