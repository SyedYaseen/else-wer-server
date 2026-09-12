use crate::api::auth_extractor::{AuthUser, StreamAuth};
use crate::api::match_meta::try_spawn_metadata_backfill;
use crate::api::middleware::{AdminUser, OrganizeUser};
use crate::db::audiobooks::{
    delete_book, get_book, get_file_path_by_id, get_files_by_book_id, list_books, reorder_files,
};
use crate::db::libraries::{get_default_library, get_library};
use crate::db::meta_scan::{cache_row_count, get_grouped_files};
use crate::file_ops::book_cover::cover_links;
use crate::file_ops::org_books::save_organized_books;
use crate::file_ops::scan_files::{scan_all_libraries, scan_files};
use crate::models::audiobooks::{DeleteBookDto, FileMetadata};
use crate::models::meta_scan::ChangeDto;
use crate::{AppState, api::api_error::ApiError};
use axum::extract::Multipart;
use axum::{
    Json,
    body::Body,
    extract::{Path, Query, Request, State},
    http::{Response, StatusCode, header},
    response::IntoResponse,
};

use sqlx::{Pool, Sqlite};
use tower::util::ServiceExt;
use tower_http::services::ServeFile;

use serde::Deserialize;
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
    OrganizeUser(_claims): OrganizeUser,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, ApiError> {
    let mut file_name = None;
    let mut chunk_index = None;
    let mut total_chunks = None;
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut folder_path = None;
    let mut library_id: Option<i64> = None;

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
        } else if name == "libraryId" {
            let text = field
                .text()
                .await
                .map_err(|e| ApiError::BadRequest(format!("Invalid libraryId field: {e}")))?;
            library_id = Some(
                text.parse::<i64>()
                    .map_err(|_| ApiError::BadRequest("libraryId must be a number".into()))?,
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

    // Chunks 1..N of the same upload must land in the same library as chunk 0 —
    // re-resolve every call (stateless across chunks) rather than trusting the
    // client to keep sending the same libraryId.
    let library = match library_id {
        Some(id) => get_library(db, id).await?,
        None => get_default_library(db).await?,
    };
    let upload_dir = &library.path;

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

        let mut count = 0u64;
        if let Some(guard) = state.scan_guard.try_start_library(library.id) {
            // Spawned so the scan (and the guard held by the task) outlives
            // this request: if the client disconnects, only our `.await`
            // below is dropped, not the scan itself, so the lock can't be
            // released before the real work is done.
            let library_id = library.id;
            let library_path = library.path.clone();
            let db_owned = db.clone();
            count = tokio::spawn(async move {
                let result = scan_files(library_id, &library_path, &db_owned).await;
                drop(guard);
                result
            })
            .await??;
            cover_links(db).await?;
            // Fire-and-forget: fills missing cover/description/series from Audible (1.5s/book).
            try_spawn_metadata_backfill(state.clone());
        } else {
            tracing::info!("Skipping post-upload rescan: a scan is already running for this library");
        }

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
    OrganizeUser(_claims): OrganizeUser,
) -> Result<impl IntoResponse, ApiError> {
    let Some(guard) = state.scan_guard.try_start_full() else {
        return Ok((
            StatusCode::CONFLICT,
            Json(json!({ "message": "A scan is already running" })),
        ));
    };

    let db = &state.db_pool;

    // Spawned so the scan (and the guard held by the task) outlives this
    // request: if the client disconnects, only our `.await` below is
    // dropped, not the scan itself, so the lock can't be released before
    // the real work is done.
    let db_owned = db.clone();
    let files_count = tokio::spawn(async move {
        let result = scan_all_libraries(&db_owned).await;
        drop(guard);
        result
    })
    .await??;

    cover_links(db).await?;
    // Fire-and-forget: fills missing cover/description/series from Audible (1.5s/book).
    try_spawn_metadata_backfill(state.clone());
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
#[derive(Deserialize)]
pub struct ListScannedFilesQuery {
    library_id: Option<i64>,
}

pub async fn list_scanned_files_handler(
    State(state): State<AppState>,
    OrganizeUser(_claims): OrganizeUser,
    Query(params): Query<ListScannedFilesQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let db = &state.db_pool;

    if cache_row_count(db, params.library_id).await? == 0 {
        // Scoped to just the requested library when filtering by one, so opening a
        // newly-added library's (empty) tab triggers only that library's scan
        // instead of a full re-scan of every library.
        let result = match params.library_id {
            Some(id) => {
                let Some(guard) = state.scan_guard.try_start_library(id) else {
                    return Ok((
                        StatusCode::CONFLICT,
                        Json(json!({ "message": "A scan is already running" })),
                    ));
                };
                // Spawned so the scan (and the guard held by the task)
                // outlives this request: if the client disconnects, only our
                // `.await` below is dropped, not the scan itself, so the
                // lock can't be released before the real work is done.
                let db_owned = db.clone();
                tokio::spawn(async move {
                    let result = match get_library(&db_owned, id).await {
                        Ok(library) => scan_files(library.id, &library.path, &db_owned).await,
                        Err(e) => Err(e),
                    };
                    drop(guard);
                    result
                })
                .await?
            }
            None => {
                let Some(guard) = state.scan_guard.try_start_full() else {
                    return Ok((
                        StatusCode::CONFLICT,
                        Json(json!({ "message": "A scan is already running" })),
                    ));
                };
                let db_owned = db.clone();
                tokio::spawn(async move {
                    let result = scan_all_libraries(&db_owned).await;
                    drop(guard);
                    result
                })
                .await?
            }
        };
        result?;
        cover_links(db).await?;
    }
    let grouped_files = get_grouped_files(db, params.library_id).await?;

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
    OrganizeUser(_claims): OrganizeUser,
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

// Admin-only: delete a book, its files on disk, and its cover-art symlink.
// Doesn't remove the files_location folder itself since it can be shared by
// sibling books (loose single-file-book folders).
pub async fn delete_book_handler(
    State(state): State<AppState>,
    AdminUser(_claims): AdminUser,
    Json(payload): Json<DeleteBookDto>,
) -> Result<impl IntoResponse, ApiError> {
    let db = &state.db_pool;
    let book = get_book(db, payload.book_id).await?; // 404 before deleting anything

    let files = get_files_by_book_id(db, payload.book_id).await?;
    for file in &files {
        if let Err(e) = fs::remove_file(&file.data.file_path).await {
            tracing::warn!(
                "delete_book: failed to remove file {}: {e}",
                file.data.file_path
            );
        }
    }

    if let Some(cover_art) = &book.cover_art
        && let Some(link_name) = cover_art.rsplit('/').next().and_then(|s| s.split('?').next())
    {
        let link_path = std::env::current_dir()?.join("covers").join(link_name);
        let _ = fs::remove_file(&link_path).await;
    }

    delete_book(db, payload.book_id).await?;

    Ok((
        StatusCode::OK,
        Json(json!({ "message": format!("Book '{}' deleted", book.title) })),
    ))
}

#[derive(Deserialize)]
pub struct ListBooksQuery {
    q: Option<String>,
    library_id: Option<i64>,
}

// List audiobooks from AudioBooks table, optionally filtered by `?q=` search term
// (matches title/author/series/narrated_by, see db::audiobooks::list_books) and/or
// `?library_id=`.
pub async fn list_books_handler(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
    Query(params): Query<ListBooksQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let db = &state.db_pool;
    let q = params.q.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let books = list_books(db, q, params.library_id).await?;
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
// Cap on how much a single Range response will return. RFC 7233 lets a server
// answer with a *narrower* range than asked for, and media elements handle that
// by issuing a follow-up request when they need more.
//
// This keeps a single request from streaming an 884 MB body: with the clamp the
// client chains 4 MiB responses instead, which costs one extra request per chunk.
// 4 MiB is >8 minutes of audio at the 64 kbps these files run at.
//
// It is *not* a fix for the lock-screen playback bug, and must not be recorded as
// one. Measurement showed the client buffers the same ~42 MB working set with or
// without the clamp — that figure is the browser's own media buffer cap. Nor is it
// the cause of the "seeker right, plays from chapter start" bug: that was measured
// to be WebKit dropping an early currentTime assignment, and reproduced with this
// clamp entirely removed. See docs/todo/11_LOCKSCREEN_RESUME.md.
const RANGE_CHUNK_BYTES: u64 = 4 * 1024 * 1024;

// Narrows an over-broad `Range` to at most RANGE_CHUNK_BYTES. Returns None — leave
// the header alone — for anything that isn't a single explicit `bytes=START-[END]`:
// suffix ranges (`bytes=-500`) and multipart ranges are rare here and not worth
// reimplementing when ServeFile already handles them correctly.
fn clamp_range(value: &str) -> Option<String> {
    let spec = value.trim().strip_prefix("bytes=")?.trim();
    if spec.contains(',') {
        return None;
    }
    let (start, end) = spec.split_once('-')?;
    let start: u64 = start.trim().parse().ok()?;
    let max_end = start.saturating_add(RANGE_CHUNK_BYTES - 1);
    match end.trim() {
        // Open-ended (`bytes=N-`, what WebKit sends) — always worth bounding.
        "" => Some(format!("bytes={start}-{max_end}")),
        // Closed (`bytes=N-M`, what Chromium sends). Honour anything already
        // within budget, including the `bytes=0-1` probe both browsers open with.
        e => {
            let end: u64 = e.parse().ok()?;
            (end > max_end).then(|| format!("bytes={start}-{max_end}"))
        }
    }
}

pub async fn stream_file(
    State(state): State<AppState>,
    StreamAuth(_claims): StreamAuth,
    Path(id): Path<i64>,
    mut req: Request, // must stay last (FromRequest) — forwards the Range header to ServeFile
) -> Result<impl IntoResponse, ApiError> {
    let file_path = get_file_path_by_id(&state.db_pool, id).await?;
    if !PathBuf::from(&file_path).exists() {
        return Err(ApiError::NotFound("File not found".into()));
    }

    let mime = audio_mime(&file_path)
        .parse::<mime::Mime>()
        .map_err(|e| ApiError::Internal(format!("bad mime: {e}")))?;

    // Temporary instrumentation for the "seeker right, plays from chapter start" investigation
    // (docs/todo/11_LOCKSCREEN_RESUME.md): logs what the client actually asked for, what the
    // clamp narrowed it to, and what ServeFile answered — so a seek that never left the client
    // can be told apart from one the client asked for correctly but iOS still didn't honour.
    // This is what identified the bug; keep it until the fix is confirmed on device.
    //
    // A rangeless GET (the offline download manager's fetch) falls through untouched and still
    // streams the whole file.
    let requested = req
        .headers()
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    // The clamp is for browsers (the PWA), which re-request the remainder after a short 206.
    // The app's player (Media3/ExoPlayer inside expo-audio) does not: it takes the 4 MiB reply
    // to an open-ended `bytes=N-` as end of file, and audio stops there while its clock keeps
    // running (device-reproduced 2026-09-12). It reads one connection at listening pace, so an
    // unclamped reply doesn't push the whole file at once. The app authenticates with an
    // Authorization header; `<audio src>` can't set one, so the PWA always uses `?token=`.
    let from_app = req.headers().contains_key(header::AUTHORIZATION);
    let clamped = if from_app {
        None
    } else {
        requested.as_deref().and_then(clamp_range)
    };
    if let Some(ref narrowed) = clamped
        && let Ok(v) = header::HeaderValue::from_str(narrowed)
    {
        req.headers_mut().insert(header::RANGE, v);
    }
    let requested_range = requested.unwrap_or_else(|| "none".to_string());
    let clamped_range = clamped.unwrap_or_else(|| "none".to_string());

    let response = ServeFile::new_with_mime(&file_path, &mime)
        .oneshot(req)
        .await
        .map_err(|e| ApiError::Internal(format!("stream error: {e}")))?;

    let status = response.status();
    let content_range = response
        .headers()
        .get(header::CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("none")
        .to_string();
    let content_length = response
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("none")
        .to_string();
    tracing::info!(
        file_id = id,
        requested_range = %requested_range,
        clamped_range = %clamped_range,
        status = %status,
        content_range = %content_range,
        content_length = %content_length,
        "stream range"
    );

    Ok(response)
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

#[derive(serde::Deserialize)]
pub struct ReorderFilesRequest {
    file_ids: Vec<i64>,
}

pub async fn reorder_files_handler(
    State(state): State<AppState>,
    OrganizeUser(_claims): OrganizeUser,
    Path(book_id): Path<i64>,
    Json(body): Json<ReorderFilesRequest>,
) -> Result<impl IntoResponse, ApiError> {
    reorder_files(&state.db_pool, book_id, &body.file_ids).await?;
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
