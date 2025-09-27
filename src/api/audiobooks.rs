use crate::api::auth_extractor::AuthUser;
use crate::db::audiobooks::{get_file_path, get_files_by_book_id, list_all_books};
use crate::db::meta_scan::group_meta_fetch;
use crate::file_ops::file_ops;
use crate::file_ops::org_books::save_organized_books;
use crate::models::audiobooks::FileMetadata;
use crate::models::meta_scan::ChangeDto;
use crate::{AppState, api::api_error::ApiError};
use axum::http::HeaderMap;
use axum::{
    Json,
    body::Body,
    extract::{Path, State},
    http::{Response, StatusCode, header},
    response::IntoResponse,
};

use serde::Deserialize;
use sqlx::{Pool, Sqlite};

use serde_json::json;
use std::io::Write;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
use zip::CompressionMethod;
use zip::write::FileOptions;

pub async fn list_books(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
) -> Result<impl IntoResponse, ApiError> {
    let db = &state.db_pool;
    match list_all_books(&db).await {
        Ok(books) => Ok(Json(json!({
            "message": "Books list",
            "count": books.len(),
            "books": books
        }))),
        Err(e) => {
            // Log the detailed error for debugging
            tracing::error!("Error scanning files: {}", e);
            Err(ApiError::Internal("Failed to scan audiobooks".to_string()))
        }
    }
}

pub async fn scan_files(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
) -> Result<impl IntoResponse, ApiError> {
    let path = &state.config.book_files;
    let db = &state.db_pool;

    let books = file_ops::scan_for_audiobooks(path, db).await?;
    Ok((
        StatusCode::OK,
        Json(json!({
            "message": "Scan completed successfully",
            "books_processed": books.len(),
            "books": books,
        })),
    ))
}

pub async fn grouped_books(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
) -> Result<impl IntoResponse, ApiError> {
    let path = &state.config.book_files;
    let db = &state.db_pool;

    // let books = file_ops::scan_for_audiobooks(path, db).await?;
    let books = group_meta_fetch(db).await?;
    // let books_json = serde_json::to_string(&books)?;
    Ok((
        StatusCode::OK,
        Json(json!({
            "message": "Scan completed successfully",
            "books_processed": books.len(),
            "books": books,
        })),
    ))
}

pub async fn confirm_books(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
    Json(payload): Json<Vec<ChangeDto>>,
) -> Result<impl IntoResponse, ApiError> {
    let db = &state.db_pool;
    let _ = save_organized_books(db, payload).await;
    Ok((
        StatusCode::OK,
        Json(json!({
            "message": "Confirmed entry",
        })),
    ))
}

pub async fn download_book(
    State(state): State<AppState>,
    Path(book_id): Path<i64>,
    AuthUser(_claims): AuthUser,
) -> impl IntoResponse {
    let files = match get_file_metadata(&state.db_pool, book_id).await {
        Ok(f) => f,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error retrieving files".to_string(),
            )
                .into_response();
        }
    };

    let mut buffer = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buffer);
        let mut zip = zip::ZipWriter::new(cursor);

        let options: FileOptions<'_, ()> = FileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .unix_permissions(0o644);

        for file in files {
            let file_name = file.data.file_path.clone();
            zip.start_file(&file_name, options).unwrap();

            // Read file content asynchronously
            if let Ok(data) = tokio::fs::read(&file_name).await {
                zip.write_all(&data).unwrap();
            }
        }
        zip.finish().unwrap();
    }

    // 3. Create Content-Disposition header
    let disposition_value = format!("attachment; filename=\"book_{}.zip\"", book_id);

    // 4. Build the response with headers
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(header::CONTENT_DISPOSITION, disposition_value)
        .body(Body::from(buffer))
        .unwrap()
}

#[derive(Debug, Deserialize)]
pub struct ChunkParams {
    pub index: i64,
    pub size: i64,
}
pub async fn download_chunk(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
    Path(file_id): Path<i64>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let file_path = get_file_path(&state.db_pool, file_id).await?;
    tracing::info!("Download init {file_path}");
    if !PathBuf::new().join(file_path.clone()).exists() {
        return Err(ApiError::BadRequest("File not found".into()));
    }

    let mut file = File::open(&file_path).await.unwrap();
    let file_size = file.metadata().await.unwrap().len();

    // parse Range header manually
    let (start, end) = if let Some(range) = headers.get("range") {
        let range_str = range.to_str().unwrap_or("");

        // Expecting "bytes=start-end"
        if let Some(range_vals) = range_str.strip_prefix("bytes=") {
            let mut parts = range_vals.split('-');
            let start: u64 = parts.next().unwrap_or("0").parse().unwrap_or(0);
            let end: u64 = parts
                .next()
                .unwrap_or(&(file_size - 1).to_string())
                .parse()
                .unwrap_or(file_size - 1);
            (start, end)
        } else {
            (0, file_size - 1)
        }
    } else {
        (0, file_size - 1)
    };

    let chunk_size = end - start + 1;
    file.seek(SeekFrom::Start(start))
        .await
        .map_err(|_| ApiError::Internal("Failed to seek in file".to_string()))?;
    let mut buffer = vec![0; chunk_size as usize];

    file.read_exact(&mut buffer)
        .await
        .map_err(|_| ApiError::Internal("Failed to read file".to_string()))?;

    let content_range = format!("bytes {}-{}/{}", start, end, file_size);

    Ok((
        StatusCode::PARTIAL_CONTENT,
        [
            ("Content-Type", "audio/mpeg".to_owned()),
            ("Content-Length", chunk_size.to_string()),
            ("Content-Range", content_range.to_owned()),
        ],
        buffer.to_owned(),
    ))
}

pub async fn file_metadata(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
    Path(book_id): Path<i64>,
) -> Result<impl IntoResponse, ApiError> {
    let files = get_file_metadata(&state.db_pool, book_id)
        .await
        .map_err(|e| {
            tracing::error!("Error scanning files: {}", e);
            ApiError::Internal("Failed to scan audiobooks".to_string())
        })?;

    Ok(Json(json!({
        "message": "",
        "count": files.len(),
        "data": files,
    })))
}

async fn get_file_metadata(db: &Pool<Sqlite>, book_id: i64) -> anyhow::Result<Vec<FileMetadata>> {
    let files = get_files_by_book_id(db, book_id).await.map_err(|e| {
        eprintln!("Error retrieving files from db: {e}");
        anyhow::anyhow!(e)
    })?;

    if files.is_empty() {
        eprintln!("No files found. BookId {}", book_id);
        return Err(anyhow::anyhow!(format!(
            "No files found. BookId {}",
            book_id
        )));
    }

    Ok(files)
}
