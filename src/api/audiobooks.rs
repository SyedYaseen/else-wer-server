use crate::api::auth_extractor::AuthUser;
use crate::db::audiobooks::{get_file_path, get_files_by_book_id, list_all_books};
use crate::db::meta_scan::{cache_row_count, get_grouped_files};
use crate::file_ops::book_cover::cover_links;
use crate::file_ops::org_books::{init_books_from_file_scan_cache, save_organized_books};
use crate::file_ops::scan_files::scan_files;
use crate::models::audiobooks::FileMetadata;
use crate::models::meta_scan::ChangeDto;
use crate::{AppState, api::api_error::ApiError};
use axum::extract::Multipart;
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
use tokio::fs::{self, File, create_dir_all, read_dir, remove_dir_all};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};
use zip::CompressionMethod;
use zip::write::FileOptions;

pub async fn upload_handler(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, ApiError> {
    let mut file_name = None;
    let mut chunk_index = None;
    let mut total_chunks = None;
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut folder_path = None;

    let upload_dir = &state.config.audiobook_location;
    let db = &state.db_pool;

    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string();
        if name == "file" {
            file_bytes = Some(field.bytes().await.unwrap().to_vec());
        } else if name == "fileName" {
            file_name = Some(field.text().await.unwrap());
        } else if name == "chunkIndex" {
            chunk_index = Some(field.text().await.unwrap().parse::<usize>().unwrap());
        } else if name == "totalChunks" {
            total_chunks = Some(field.text().await.unwrap().parse::<usize>().unwrap());
        } else if name == "folderPath" {
            folder_path = Some(field.text().await.unwrap());
        }
    }

    let file_name = file_name.ok_or(ApiError::BadRequest("Missing fileName".to_owned()))?;
    let chunk_index = chunk_index.ok_or(ApiError::BadRequest("Missing chunkIndex".to_owned()))?;
    let total_chunks =
        total_chunks.ok_or(ApiError::BadRequest("Missing totalChunks".to_owned()))?;
    let file_bytes = file_bytes.ok_or(ApiError::BadRequest("Missing file data".to_owned()))?;
    let folder_path = folder_path.ok_or(ApiError::BadRequest("Missing fileName".to_owned()))?;

    let parts_dir = format!("{upload_dir}/{file_name}.parts");
    // Create temp dir per file
    if chunk_index == 0 {
        create_dir_all(&parts_dir).await?;
    }

    // Save chunk
    let chunk_path = format!("{parts_dir}/{chunk_index}");
    let mut f = File::create(&chunk_path).await?;
    f.write(&file_bytes).await?;

    let mut item_count = 0;
    let mut entries = read_dir(&parts_dir).await?;

    while let Ok(Some(_entry)) = entries.next_entry().await {
        item_count += 1;
    }

    if item_count == total_chunks {
        let target_folder = format!("{upload_dir}/{folder_path}");
        create_dir_all(&target_folder).await?;

        let final_path = format!("{target_folder}{file_name}");
        println!("{final_path}");
        let mut output = fs::File::create(&final_path).await?;
        for i in 0..total_chunks {
            let chunk_path = format!("{parts_dir}/{i}");
            let mut chunk_file = fs::File::open(&chunk_path).await?;
            let mut buf = Vec::new();
            chunk_file.read_to_end(&mut buf).await?;
            output.write_all(&buf).await?;
        }

        // cleanup
        remove_dir_all(&parts_dir).await?;
        println!("✅ File saved to {final_path}");
        let count = scan_files(upload_dir, db).await?;

        return Ok((
            StatusCode::OK,
            Json(json!({
                "index": chunk_index,
                "upload_complete": true,
                "num_files": count
            })),
        ));
    }

    Ok((
        StatusCode::OK,
        Json(json!({
            "index": chunk_index,
            "upload_complete": false,
            "num_files": 0
        })),
    ))
}

// Scan all audiobook files on local hard drive
pub async fn scan_files_handler(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
) -> Result<impl IntoResponse, ApiError> {
    let path = &state.config.audiobook_location;
    let db = &state.db_pool;

    let files_count = scan_files(path, db).await?;
    Ok((
        StatusCode::OK,
        Json(json!({
            "message": "Scan completed successfully",
            "files_scanned": files_count,
        })),
        // TODO: Append failed scan locations into a warn/ err array response
    ))
}

// Get list of all audiobookfiles grouped by author -> book -> files
pub async fn list_scanned_files_handler(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
) -> Result<impl IntoResponse, ApiError> {
    let path = &state.config.audiobook_location;
    let db = &state.db_pool;

    if cache_row_count(db).await? == 0 {
        scan_files(path, db).await?;
    }
    let grouped_files = get_grouped_files(db).await?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "files": grouped_files,
        })),
    ))
}

// Save organization made by user on their local audiofiles
pub async fn save_organized_files_handler(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
    Json(payload): Json<Vec<ChangeDto>>,
) -> Result<impl IntoResponse, ApiError> {
    let db = &state.db_pool;
    let _ = save_organized_books(db, payload).await;
    cover_links(db).await?;
    Ok((
        StatusCode::OK,
        Json(json!({
            "message": "Confirmed entry",
        })),
    ))
}

pub async fn init_books_from_file_scan_cache_handler(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
) -> Result<impl IntoResponse, ApiError> {
    let db = &state.db_pool;
    let _ = init_books_from_file_scan_cache(db).await;
    cover_links(db).await?;
    Ok((
        StatusCode::OK,
        Json(json!({
            "message": "Confirmed entry",
        })),
    ))
}

// List audiobooks from AudioBooks table
pub async fn list_books_handler(
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

// User downloads entire book
pub async fn download_book(
    State(state): State<AppState>,
    Path(book_id): Path<i64>,
    AuthUser(_claims): AuthUser,
) -> impl IntoResponse {
    let files = match file_metadata(&state.db_pool, book_id).await {
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

pub async fn file_metadata_handler(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
    Path(book_id): Path<i64>,
) -> Result<impl IntoResponse, ApiError> {
    let files = file_metadata(&state.db_pool, book_id).await.map_err(|e| {
        tracing::error!("Error scanning files: {}", e);
        ApiError::Internal("Failed to scan audiobooks".to_string())
    })?;

    Ok(Json(json!({
        "message": "",
        "count": files.len(),
        "data": files,
    })))
}

async fn file_metadata(db: &Pool<Sqlite>, book_id: i64) -> anyhow::Result<Vec<FileMetadata>> {
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
