use axum::{
    Router,
    extract::State,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use tower_http::services::ServeDir;
mod api_error;
mod audiobooks;
mod auth_extractor;
mod middleware;
mod sync;
pub mod user;
use crate::{
    AppState,
    api::{
        audiobooks::{download_book, file_metadata, list_books},
        sync::{get_book_progress, get_file_progress, update_progress},
        user::{create_user, login},
    },
};
use audiobooks::scan_files;

pub async fn routes() -> Router<AppState> {
    Router::new()
        .nest_service("/covers", ServeDir::new("covers"))
        .route("/hello", get(hello))
        // Books
        .route("/scan_files", get(scan_files))
        .route("/list_books", get(list_books))
        // Files
        .route("/download_book/{book_id}", get(download_book))
        .route("/file_metadata/{book_id}", get(file_metadata))
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

    Html("<h1>Hello</h1>")
}
