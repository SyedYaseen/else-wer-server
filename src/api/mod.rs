use axum::{
    Router,
    extract::State,
    response::{Html, IntoResponse},
    routing::{get, post},
};
mod api_error;
mod audiobooks;
mod sync;
use crate::{
    AppState,
    api::{
        audiobooks::{download_book, file_metadata, list_books},
        sync::{get_progress, update_progress},
    },
};
use audiobooks::scan_files;
pub async fn routes() -> Router<AppState> {
    Router::new()
        .route("/hello", get(hello))
        // Books
        .route("/scan_files", get(scan_files))
        .route("/list_books", get(list_books))
        // Files
        .route("/download_book/{fileid}", get(download_book))
        .route("/file_metadata/{book_id}", get(file_metadata))
        // Sync
        .route(
            "/get_progress/{user_id}/{book_id}/{file_id}",
            get(get_progress),
        )
        .route("/update_progress", post(update_progress))
}

async fn hello(State(state): State<AppState>) -> impl IntoResponse {
    println!("{}", state.config.book_files);
    Html("<h1>Hello</h1>")
}
