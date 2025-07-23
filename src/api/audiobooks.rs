use crate::file_ops::file_ops;
use crate::{AppState, api::api_error::ApiError};
use axum::Json;
use axum::{extract::State, response::IntoResponse};
use serde_json::json;

pub async fn scan_files(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let path = &state.config.book_files;
    let db = &state.db_pool;

    match file_ops::scan_for_audiobooks(path, db).await {
        Ok(books) => Ok(Json(json!({
            "message": "Scan completed successfully",
            "books_processed": books.len(),
            "books": books,
        }))),
        Err(e) => {
            // Log the detailed error for debugging
            tracing::error!("Error scanning files: {}", e);
            Err(ApiError::InternalServerError(
                "Failed to scan audiobooks".to_string(),
            ))
        }
    }
}
