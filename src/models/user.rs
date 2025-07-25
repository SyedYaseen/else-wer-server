use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub salt: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Progress {
    pub id: i64,
    pub user_id: i64,
    pub book_id: i64,
    pub file_id: i64,
    pub progress_time_marker: i64,
}
