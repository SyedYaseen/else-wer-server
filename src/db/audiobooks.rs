use crate::{
    api::api_error::ApiError,
    models::audiobooks::{AudioBook, AudioBookRow, CreateFileMetadata, FileMetadata},
};
use sqlx::{Pool, Sqlite};

pub async fn list_all_books(db: &Pool<Sqlite>) -> Result<Vec<AudioBookRow>, ApiError> {
    let books = sqlx::query_as::<_, AudioBookRow>(
        r#"
        SELECT id, author, series, title, files_location, cover_art, duration, metadata
        FROM audiobooks
        ORDER BY author, series, title
        "#,
    )
    .fetch_all(db)
    .await?;

    Ok(books)
}

pub async fn insert_audiobook(db: &Pool<Sqlite>, book: &AudioBook) -> Result<i64, ApiError> {
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

pub async fn update_audiobook_duration(
    db: &Pool<Sqlite>,
    bookid: i64,
    duration: i64,
) -> Result<(), ApiError> {
    sqlx::query!(
        r#"
        UPDATE audiobooks
        SET duration = ?1
        WHERE id = ?2
        "#,
        duration,
        bookid
    )
    .execute(db)
    .await?;

    Ok(())
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

pub async fn insert_file_metadata(
    db: &Pool<Sqlite>,
    create_data: &mut CreateFileMetadata,
) -> Result<(), ApiError> {
    // let file_path = create_data.file_path.to_string().to_owned();

    sqlx::query!(
        r#"
        INSERT INTO files (book_id, file_id, file_name, file_path, duration, channels, sample_rate, bitrate)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        create_data.book_id,
        create_data.file_id,
        create_data.file_name,
        create_data.file_path,
        create_data.duration,
        create_data.channels,
        create_data.sample_rate,
        create_data.bitrate
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn get_audiobook_id(db: &Pool<Sqlite>, book: &AudioBook) -> Result<i64, ApiError> {
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

// pub async fn get_audiobook_by_id(db: &Pool<Sqlite>, bookid: i64) -> Result<AudioBookRow> {
//     let row = sqlx::query_as(
//         r#"
//         SELECT *
//         FROM audiobooks
//         WHERE id = ?1
//         "#,
//     )
//     .bind(bookid)
//     .fetch_one(db)
//     .await?;

//     Ok(row)
// }

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
        .map(|r| FileMetadata {
            id: r.id.expect("Id doesnt exist"),
            data: CreateFileMetadata {
                book_id: r.book_id,
                file_id: Some(r.file_id),
                file_name: r.file_name,
                file_path: r.file_path,
                duration: r.duration,
                channels: r.channels,
                sample_rate: r.sample_rate,
                bitrate: r.bitrate,
            },
        })
        .collect();

    Ok(files)
}

pub async fn get_file_path(db: &Pool<Sqlite>, file_id: i64) -> Result<String, ApiError> {
    let path: (String,) = sqlx::query_as(
        r#"
        SELECT
            file_path
        FROM files
        WHERE file_id = ?
        ORDER BY id
        "#,
    )
    .bind(file_id)
    .fetch_one(db)
    .await?;

    Ok(path.0)
}
