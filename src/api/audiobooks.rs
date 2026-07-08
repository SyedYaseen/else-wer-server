use crate::api::auth_extractor::AuthUser;
use crate::db::audiobooks::{get_file_path_by_id, get_files_by_book_id, list_all_books};
use crate::db::meta_scan::{cache_row_count, get_grouped_files};
use crate::file_ops::book_cover::cover_links;
use crate::file_ops::org_books::save_organized_books;
use crate::file_ops::scan_files::scan_files;
use crate::models::audiobooks::FileMetadata;
use crate::models::meta_scan::ChangeDto;
use crate::{AppState, api::api_error::ApiError};
use axum::extract::Multipart;
use axum::{
    Json,
    body::Body,
    extract::{Path, Request, State},
    http::{Response, StatusCode, header},
    response::IntoResponse,
};

use sqlx::{Pool, Sqlite};
use tower::util::ServiceExt;
use tower_http::services::ServeFile;

use serde_json::json;
use std::io::Write;
use std::path::PathBuf;
use tokio::fs::{self, File, create_dir_all, read_dir, remove_dir_all};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use zip::CompressionMethod;
use zip::write::FileOptions;

fn is_safe_filename(name: &str) -> bool {
    !name.is_empty() && !name.contains('/') && !name.contains('\\') && name != "." && name != ".."
}

fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty() && !path.starts_with('/') && !path.contains("..") && !path.contains('\\')
}

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

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(format!("Invalid multipart data: {e}")))?
    {
        let name = field
            .name()
            .ok_or_else(|| ApiError::BadRequest("Multipart field missing name".into()))?
            .to_string();
        if name == "file" {
            file_bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("Invalid file field: {e}")))?
                    .to_vec(),
            );
        } else if name == "fileName" {
            file_name = Some(
                field
                    .text()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("Invalid fileName field: {e}")))?,
            );
        } else if name == "chunkIndex" {
            chunk_index = Some(
                field
                    .text()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("Invalid chunkIndex field: {e}")))?
                    .parse::<usize>()
                    .map_err(|_| ApiError::BadRequest("chunkIndex must be a number".into()))?,
            );
        } else if name == "totalChunks" {
            total_chunks = Some(
                field
                    .text()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("Invalid totalChunks field: {e}")))?
                    .parse::<usize>()
                    .map_err(|_| ApiError::BadRequest("totalChunks must be a number".into()))?,
            );
        } else if name == "folderPath" {
            folder_path = Some(
                field
                    .text()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("Invalid folderPath field: {e}")))?,
            );
        }
    }

    let (file_name, chunk_index, total_chunks, file_bytes, folder_path) = (
        file_name.ok_or_else(|| ApiError::BadRequest("Missing fileName".into()))?,
        chunk_index.ok_or_else(|| ApiError::BadRequest("Missing chunkIndex".into()))?,
        total_chunks.ok_or_else(|| ApiError::BadRequest("Missing totalChunks".into()))?,
        file_bytes.ok_or_else(|| ApiError::BadRequest("Missing file data".into()))?,
        folder_path.ok_or_else(|| ApiError::BadRequest("Missing folderPath".into()))?,
    );

    if !is_safe_filename(&file_name) {
        return Err(ApiError::BadRequest("Invalid fileName".into()));
    }
    if !is_safe_relative_path(&folder_path) {
        return Err(ApiError::BadRequest("Invalid folderPath".into()));
    }

    let parts_dir = format!("{upload_dir}/{file_name}.parts");

    // Create temp dir per file
    if chunk_index == 0 {
        create_dir_all(&parts_dir).await?;
    }

    // Save chunk
    let chunk_path = format!("{parts_dir}/{chunk_index}");
    let mut f = File::create(&chunk_path).await?;
    f.write_all(&file_bytes).await?;

    let mut item_count = 0;
    let mut entries = read_dir(&parts_dir).await?;

    while let Ok(Some(_entry)) = entries.next_entry().await {
        item_count += 1;
    }

    if item_count == total_chunks {
        let target_folder = format!("{upload_dir}/{folder_path}");
        create_dir_all(&target_folder).await?;

        let final_path = format!("{target_folder}{file_name}");
        tracing::debug!("File final path: {final_path}");
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
        tracing::info!("File saved to {final_path}");
        let count = scan_files(upload_dir, db).await?;
        cover_links(db).await?;

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
    cover_links(db).await?;
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
        cover_links(db).await?;
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
    save_organized_books(db, payload).await?;
    // cover_links(db).await?;
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
    let books = list_all_books(&db).await?;
    Ok(Json(json!({
        "message": "Books list",
        "count": books.len(),
        "books": books
    })))
}

// User downloads entire book
pub async fn download_book(
    State(state): State<AppState>,
    Path(book_id): Path<i64>,
    AuthUser(_claims): AuthUser,
) -> Result<impl IntoResponse, ApiError> {
    let files = file_metadata(&state.db_pool, book_id).await?;

    let mut buffer = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buffer);
        let mut zip = zip::ZipWriter::new(cursor);

        let options: FileOptions<'_, ()> = FileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .unix_permissions(0o644);

        for file in files {
            let file_name = file.data.file_path.clone();
            zip.start_file(&file_name, options)?;

            // Read file content asynchronously
            if let Ok(data) = tokio::fs::read(&file_name).await {
                zip.write_all(&data)?;
            }
        }
        zip.finish()?;
    }

    // 3. Create Content-Disposition header
    let disposition_value = format!("attachment; filename=\"book_{}.zip\"", book_id);

    // 4. Build the response with headers
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(header::CONTENT_DISPOSITION, disposition_value)
        .body(Body::from(buffer))?;

    Ok(response)
}

// Content-Type by extension; mime_guess (used inside ServeFile) doesn't map .m4b,
// and players are picky about audio mime types.
fn audio_mime(path: &str) -> &'static str {
    match std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("mp3") => "audio/mpeg",
        Some("m4a") | Some("m4b") | Some("mp4") => "audio/mp4",
        Some("aac") => "audio/aac",
        Some("flac") => "audio/flac",
        Some("ogg") | Some("oga") | Some("opus") => "audio/ogg",
        Some("wav") => "audio/wav",
        _ => "application/octet-stream",
    }
}

// GET/HEAD /api/stream/{id} — id is the files PK. ServeFile provides RFC 7233
// Range support (206/416, Accept-Ranges, If-Range, HEAD) with a streamed body;
// used by both app streaming playback and the chunked download manager.
pub async fn stream_file(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
    Path(id): Path<i64>,
    req: Request, // must stay last (FromRequest) — forwards the Range header to ServeFile
) -> Result<impl IntoResponse, ApiError> {
    let file_path = get_file_path_by_id(&state.db_pool, id).await?;
    if !PathBuf::from(&file_path).exists() {
        return Err(ApiError::NotFound("File not found".into()));
    }

    let mime = audio_mime(&file_path)
        .parse::<mime::Mime>()
        .map_err(|e| ApiError::Internal(format!("bad mime: {e}")))?;

    ServeFile::new_with_mime(&file_path, &mime)
        .oneshot(req)
        .await
        .map_err(|e| ApiError::Internal(format!("stream error: {e}")))
}

pub async fn file_metadata_handler(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
    Path(book_id): Path<i64>,
) -> Result<impl IntoResponse, ApiError> {
    let files = file_metadata(&state.db_pool, book_id).await?;

    Ok(Json(json!({
        "message": "",
        "count": files.len(),
        "data": files,
    })))
}

async fn file_metadata(db: &Pool<Sqlite>, book_id: i64) -> Result<Vec<FileMetadata>, ApiError> {
    let files = get_files_by_book_id(db, book_id).await?;

    if files.is_empty() {
        return Err(ApiError::NotFound(format!(
            "No files found. BookId {}",
            book_id
        )));
    }

    Ok(files)
}
