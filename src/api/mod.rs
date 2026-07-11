use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    response::{Html, IntoResponse},
    routing::{get, post, put},
};
use serde_json::json;
use tower_http::services::ServeDir;
pub mod api_error;
mod audiobooks;
mod auth_extractor;
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
        match_meta::{apply_book_match, assign_series, backfill_metadata, match_book_candidates},
        stats::{get_daily_stats, get_finished_books},
        sync::{get_book_progress, get_file_progress, list_inprogress, update_progress},
        user::{
            change_password, create_user, delete_user, list_users, login,
            update_user_permissions,
        },
    },
};

use audiobooks::scan_files_handler;

pub async fn routes() -> Router<AppState> {
    Router::new()
        .nest_service("/covers", ServeDir::new("covers"))
        .route("/hello", get(hello))
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
        .layer(DefaultBodyLimit::max(1024 * 1024 * 10))
}

// Unauthenticated liveness probe backing the client's server-reachability
// indicator. No AuthUser extractor and no State access, so it's cheap enough
// to poll every few seconds indefinitely. Deliberately separate from /hello
// (a scratch/debug endpoint, not stable infra).
async fn health_handler() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

async fn hello(State(_state): State<AppState>) -> impl IntoResponse {
    // println!(
    //     "Hello {}, role: {} id: {}",
    //     claims.username, claims.role, claims.sub
    // );
    // println!("{}", state.config.book_files);
    // let curr_dir = std::env::current_dir().unwrap();
    // let src_p = "data/AdrianTchaikovsky/Elder Race [2021]/cover.jpg";
    // let dest_p = "covers/test.jpg";

    // let source = std::env::current_dir().unwrap().join(&src_p);
    // let target = curr_dir.clone().join(&dest_p);
    // info!("IN hello endpoint");
    // tracing::error!("IN hello endpoint");
    // ApiError::Internal("Soething went".to_string())
    Html("<h1>Hello</h1>")
}
