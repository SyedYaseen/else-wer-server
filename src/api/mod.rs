use axum::{
    Router,
    extract::{DefaultBodyLimit, State},
    response::{Html, IntoResponse},
    routing::{get, post},
};
use tower_http::services::ServeDir;
pub mod api_error;
mod audiobooks;
mod auth_extractor;
mod middleware;
mod sync;
pub mod user;
use crate::{
    AppState,
    api::{
        audiobooks::{
            download_book, download_chunk, file_metadata_handler, list_books_handler,
            list_scanned_files_handler, save_organized_files_handler, upload_handler,
        },
        sync::{get_book_progress, get_file_progress, update_progress},
        user::{create_user, login},
    },
};

use audiobooks::scan_files_handler;

pub async fn routes() -> Router<AppState> {
    Router::new()
        .nest_service("/covers", ServeDir::new("covers"))
        .route("/hello", get(hello))
        // bookscan + edit
        .route("/scan_files", get(scan_files_handler))
        .route("/list_scanned_files", get(list_scanned_files_handler))
        .route("/save_organized_files", post(save_organized_files_handler))
        // upload
        .route("/upload", post(upload_handler))
        // Books
        .route("/list_books", get(list_books_handler))
        // Files
        .route("/download_book/{book_id}", get(download_book)) // TODO: This might be obsolete
        .route("/download_chunk/{file_id}", get(download_chunk))
        .route("/file_metadata/{book_id}", get(file_metadata_handler))
        // Sync
        .route(
            "/get_file_progress/{book_id}/{file_id}",
            get(get_file_progress),
        )
        .route("/get_book_progress/{book_id}", get(get_book_progress))
        .route("/update_progress", post(update_progress))
        // User
        .route("/create_user", post(create_user))
        .route("/login", post(login))
        .layer(DefaultBodyLimit::max(1024 * 1024 * 10))
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
