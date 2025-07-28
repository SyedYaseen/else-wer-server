use crate::db::audiobooks::{get_files_by_book_id, list_all_books};
use crate::file_ops::file_ops;
use crate::models::audiobooks::FileMetadata;
use crate::{AppState, api::api_error::ApiError};
use Result::Ok;
use anyhow::anyhow;
use axum::body::Body;
use axum::{
    Json,
    extract::{Path, State},
    http::{Response, StatusCode, header},
    response::IntoResponse,
};
use sqlx::{Pool, Sqlite, pool};

use serde_json::json;
use std::io::Write;
use zip::CompressionMethod;
use zip::write::FileOptions;

pub async fn list_books(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
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
            Err(ApiError::InternalServerError(
                "Failed to scan audiobooks".to_string(),
            ))
        }
    }
}

pub async fn scan_files(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let path = &state.config.book_files;
    let db = &state.db_pool;

    match file_ops::scan_for_audiobooks(path, db).await {
        Ok(books) => Ok(Json(json!({
            "message": "Scan completed successfully",
            "books_processed": books.len(),
            "books": books,
        }))),
        Err(e) => {
            // Log the detailed error for debugging
            tracing::error!("Error scanning files: {}", e);
            Err(ApiError::InternalServerError(
                "Failed to scan audiobooks".to_string(),
            ))
        }
    }
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

pub async fn download_book(
    State(state): State<AppState>,
    Path(book_id): Path<i64>,
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

pub async fn file_metadata(
    State(state): State<AppState>,
    Path(book_id): Path<i64>,
) -> Result<impl IntoResponse, ApiError> {
    match get_file_metadata(&state.db_pool, book_id).await {
        Ok(files) => Ok(Json(json!({
            "message": "",
            "count": files.len(),
            "data": files,
        }))),
        Err(e) => {
            tracing::error!("Error scanning files: {}", e);
            Err(ApiError::InternalServerError(
                "Failed to scan audiobooks".to_string(),
            ))
        }
    }
}
