use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Bookmark {
    pub id: i64,
    pub user_id: i64,
    pub book_id: i64,
    pub file_id: i64,
    pub timestamp_ms: i64,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBookmark {
    pub book_id: i64,
    pub file_id: i64,
    pub timestamp_ms: i64,
    #[serde(default)]
    pub note: Option<String>,
}
