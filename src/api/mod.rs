use axum::{
    Router,
    extract::State,
    response::{Html, IntoResponse},
    routing::get,
};
mod api_error;
mod audiobooks;
use crate::AppState;
use audiobooks::scan_files;
pub async fn routes() -> Router<AppState> {
    Router::new()
        .route("/hello", get(hello))
        .route("/scan_files", get(scan_files))
}

async fn hello(State(state): State<AppState>) -> impl IntoResponse {
    println!("{}", state.config.book_files);
    Html("<h1>Hello</h1>")
}
