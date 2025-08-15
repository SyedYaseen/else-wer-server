use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
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
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        tracing::error!("API error: {:?}", self);

        let (status, error_message) = match &self {
            ApiError::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error".to_string(),
            ),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
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
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
