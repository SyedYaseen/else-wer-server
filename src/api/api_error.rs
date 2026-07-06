use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use lofty::error::LoftyError;
use serde_json::json;
use thiserror::Error;
use tokio::task::JoinError;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Database error")]
    Database(#[from] sqlx::Error),

    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("JWT error")]
    JwtErr(#[from] jsonwebtoken::errors::Error),

    #[error("Password hash error")]
    PasswordErr(#[from] argon2::password_hash::Error),

    #[error("Custom IO error: {0}")]
    IOErrCustom(String),

    #[error("IO error: {0}")]
    IOErr(#[from] std::io::Error),

    #[error("Join error: {0}")]
    JoinErr(#[from] JoinError),

    #[error("Walkdir error: {0}")]
    WalkDirErr(#[from] walkdir::Error),

    #[error("Lofty error: {0}")]
    LoftyErr(#[from] LoftyError),

    #[error("Serde Json: {0}")]
    JsonErr(#[from] serde_json::Error),

    #[error("Fetch failed: {0}")]
    ReqwestErr(#[from] reqwest::Error),

    #[error("Zip error: {0}")]
    ZipErr(#[from] zip::result::ZipError),

    #[error("HTTP response build error: {0}")]
    HttpErr(#[from] axum::http::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            ApiError::Database(sqlx::Error::RowNotFound) => {
                (StatusCode::NOT_FOUND, "Not found".to_string())
            }
            ApiError::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error".to_string(),
            ),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            ApiError::JwtErr(_) => (
                StatusCode::UNAUTHORIZED,
                "Invalid or expired token".to_string(),
            ),
            ApiError::PasswordErr(_) => {
                (StatusCode::UNAUTHORIZED, "Invalid credentials".to_string())
            }
            ApiError::IOErr(_e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Something went wrong when accessing your file system".to_string(),
            ),
            ApiError::IOErrCustom(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ApiError::JoinErr(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Err while combining threads".to_string(),
            ),
            ApiError::WalkDirErr(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Err while reading files".to_string(),
            ),
            ApiError::LoftyErr(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Err while reading files".to_string(),
            ),
            ApiError::JsonErr(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Err while serialization".to_string(),
            ),
            ApiError::ReqwestErr(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to reach an external server".to_string(),
            ),
            ApiError::ZipErr(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Err while building archive".to_string(),
            ),
            ApiError::HttpErr(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Err while building response".to_string(),
            ),
        };

        if status.is_server_error() {
            tracing::error!("API error: {:?}", self);
        } else {
            tracing::debug!("API error: {:?}", self);
        }

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
