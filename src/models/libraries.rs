use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Library {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateLibraryDto {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLibraryDto {
    pub name: Option<String>,
    pub path: Option<String>,
    #[serde(default)]
    pub is_default: Option<bool>,
}
