use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

#[derive(Debug, Serialize, Deserialize)]
pub struct UserDto {
    pub username: String,
    pub password: String,
    pub is_admin: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginDto {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
    pub password_hash: String,
    pub salt: String,
}

#[derive(sqlx::FromRow, Serialize, Deserialize, Debug)]
pub struct Progress {
    pub id: i64,
    pub user_id: i64,
    pub book_id: i64,
    pub file_id: i64,
    pub progress_ms: i64,
    pub complete: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ProgressUpdate {
    pub user_id: i64,
    pub book_id: i64,
    pub file_id: i64,
    pub progress_ms: i64,
    pub complete: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i64,         // subject, usually user ID
    pub role: String,     // "admin" or "user"
    pub username: String, // optional additional info
    pub exp: usize,       // expiration timestamp (seconds since epoch)
    pub iat: usize,       // issued at timestamp
}
