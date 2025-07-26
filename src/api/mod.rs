use axum::{
    Router,
    extract::State,
    response::{Html, IntoResponse},
    routing::get,
};
mod api_error;
mod audiobooks;
use crate::{
    AppState,
    api::audiobooks::{download_book, list_books},
};
use audiobooks::scan_files;
pub async fn routes() -> Router<AppState> {
    Router::new()
        .route("/hello", get(hello))
        .route("/scan_files", get(scan_files))
        .route("/download_book/{fileid}", get(download_book))
        .route("/list_books", get(list_books))
}

async fn hello(State(state): State<AppState>) -> impl IntoResponse {
    println!("{}", state.config.book_files);
    Html("<h1>Hello</h1>")
}
