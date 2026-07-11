use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

#[derive(Debug, Serialize, Deserialize)]
pub struct UserDto {
    pub username: String,
    pub password: String,
    pub is_admin: bool,
    #[serde(default)]
    pub can_organize: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginDto {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChangePasswordDto {
    pub username: String,
    pub new_password: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
    pub password_hash: String,
    pub can_organize: bool,
    pub token_version: i64,
}

/// Public-facing user shape for the admin dashboard — never leaks password_hash/salt.
#[derive(Debug, Serialize)]
pub struct UserSummary {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
    pub can_organize: bool,
}

impl From<User> for UserSummary {
    fn from(u: User) -> Self {
        UserSummary {
            id: u.id,
            username: u.username,
            is_admin: u.is_admin,
            can_organize: u.can_organize,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateUserPermissionsDto {
    pub user_id: i64,
    pub is_admin: bool,
    pub can_organize: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteUserDto {
    pub user_id: i64,
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
    // Defaulted so tokens issued before this field existed still decode (as false,
    // the safe default) instead of failing auth outright.
    #[serde(default)]
    pub can_organize: bool,
    // Compared against the user's current token_version in the DB on every request;
    // a mismatch means the token was issued before a password change and is revoked.
    // Defaulted to 0 so pre-existing tokens (issued before this field existed) still
    // decode and match a freshly-migrated user row (which also defaults to 0).
    #[serde(default)]
    pub token_version: i64,
}
