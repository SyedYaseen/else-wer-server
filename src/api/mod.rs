use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use serde_json::json;
use std::time::Duration;
use tokio::time::timeout;
use tower_http::services::ServeDir;
pub mod api_error;
mod audiobooks;
mod auth_extractor;
mod bookmarks;
mod libraries;
mod match_meta;
mod middleware;
pub mod rate_limit;
mod stats;
mod sync;
pub mod user;
use crate::{
    AppState,
    api::{
        audiobooks::{
            delete_book_handler, download_book, file_metadata_handler, list_books_handler,
            list_scanned_files_handler, reorder_files_handler, save_organized_files_handler,
            stream_file, upload_handler,
        },
        bookmarks::{create_bookmark_handler, delete_bookmark_handler, list_bookmarks_handler},
        libraries::{
            create_library_handler, delete_library_handler, list_libraries_handler,
            scan_library_handler, update_library_handler,
        },
        match_meta::{apply_book_match, assign_series, backfill_metadata, match_book_candidates},
        stats::{get_daily_stats, get_finished_books},
        sync::{get_book_progress, get_file_progress, list_inprogress, update_progress},
        user::{
            change_password, create_user, delete_user, list_users, login, refresh_token,
            update_user_permissions,
        },
    },
};

use audiobooks::scan_files_handler;

pub async fn routes() -> Router<AppState> {
    Router::new()
        .nest_service("/covers", ServeDir::new("covers"))
        .route("/health", get(health_handler))
        // bookscan + edit
        .route("/scan_files", get(scan_files_handler))
        .route("/list_scanned_files", get(list_scanned_files_handler))
        .route("/save_organized_files", post(save_organized_files_handler))
        // upload
        .route("/upload", post(upload_handler))
        // Books
        .route("/list_books", get(list_books_handler))
        // External metadata match (explicit user action; GET is read-only)
        .route(
            "/match_book/{book_id}",
            get(match_book_candidates).post(apply_book_match),
        )
        .route("/backfill_metadata", post(backfill_metadata))
        .route("/assign_series", post(assign_series))
        .route("/delete_book", post(delete_book_handler))
        // Files
        .route("/download_book/{book_id}", get(download_book)) // TODO: This might be obsolete
        .route("/stream/{id}", get(stream_file)) // GET also matches HEAD; ServeFile handles it
        .route("/file_metadata/{book_id}", get(file_metadata_handler))
        .route(
            "/file_metadata/{book_id}/reorder",
            post(reorder_files_handler),
        )
        // Sync
        .route("/list_inprogress", get(list_inprogress))
        .route(
            "/get_file_progress/{book_id}/{file_id}",
            get(get_file_progress),
        )
        .route("/get_book_progress/{book_id}", get(get_book_progress))
        .route("/update_progress", post(update_progress))
        // Bookmarks
        .route("/bookmarks", post(create_bookmark_handler))
        .route("/bookmarks/book/{book_id}", get(list_bookmarks_handler))
        .route(
            "/bookmarks/{bookmark_id}",
            delete(delete_bookmark_handler),
        )
        // Libraries
        .route(
            "/libraries",
            get(list_libraries_handler).post(create_library_handler),
        )
        .route(
            "/libraries/{id}",
            put(update_library_handler).delete(delete_library_handler),
        )
        .route("/libraries/{id}/scan", post(scan_library_handler))
        // Stats
        .route("/stats/daily", get(get_daily_stats))
        .route("/stats/finished", get(get_finished_books))
        // User
        .route("/create_user", post(create_user))
        .route("/list_users", get(list_users))
        .route("/delete_user", post(delete_user))
        .route("/update_user_permissions", post(update_user_permissions))
        .route("/user/change_password", put(change_password))
        .route("/login", post(login))
        .route("/refresh_token", post(refresh_token))
        .layer(DefaultBodyLimit::max(1024 * 1024 * 10))
}

// Unauthenticated liveness probe backing the client's server-reachability
// indicator. No AuthUser extractor, and the only state access is one small
// indexed SELECT, so it's still cheap enough to poll every few seconds
// indefinitely.
//
// Always 200, including when a library's disk is unreachable: the client only
// inspects res.ok, so a non-2xx here would surface as "can't reach the server"
// and bury the more specific media_ok signal we're adding. media_ok is null when
// the check couldn't be completed.
async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    match unavailable_libraries(&state.db_pool).await {
        Some(names) => Json(json!({
            "status": "ok",
            "media_ok": names.is_empty(),
            "media_unavailable": names,
        })),
        None => Json(json!({ "status": "ok", "media_ok": null })),
    }
}

/// Names of libraries whose root isn't currently a reachable directory — the signal
/// that a media drive has been unmounted. Same reachability notion `scan_files` uses
/// before it refuses to treat an unreachable path as a mass deletion.
///
/// `None` means the check itself couldn't run, reported as "unknown" rather than
/// "broken" so a transient DB hiccup never shows a scary banner.
///
/// Names, not paths: this endpoint is unauthenticated, and the name is enough to say
/// which library is affected without publishing the server's directory layout.
///
/// tokio::fs behind a timeout rather than `Path::is_dir()` — a wedged USB bus can make
/// a stat block, and blocking here would stall the runtime and blow the client's 2.5s
/// probe timeout, turning a degraded drive into a false "server unreachable".
async fn unavailable_libraries(db: &sqlx::SqlitePool) -> Option<Vec<String>> {
    const STAT_TIMEOUT: Duration = Duration::from_millis(500);

    let libraries = crate::db::libraries::list_libraries(db).await.ok()?;
    let mut unavailable = Vec::new();
    for library in libraries {
        let reachable = timeout(STAT_TIMEOUT, tokio::fs::metadata(&library.path))
            .await
            .is_ok_and(|res| res.is_ok_and(|meta| meta.is_dir()));
        if !reachable {
            unavailable.push(library.name);
        }
    }
    Some(unavailable)
}

