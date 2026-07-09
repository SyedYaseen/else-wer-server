use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct AudioBookRow {
    pub id: i64,
    pub author: String,
    pub series: Option<String>,
    pub title: String,
    pub files_location: String,
    pub duration: i64,
    pub cover_art: Option<String>,
    pub metadata: Option<String>,
    pub book_size: i64,
    pub series_id: Option<i64>,
    pub series_sequence: Option<String>,
    pub asin: Option<String>,
    pub narrated_by: Option<String>,
    pub series_name: Option<String>,
    pub user_locked: bool,
    pub description: Option<String>,
    pub series_locked: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BaseFileMetadata {
    pub book_id: i64,
    pub file_id: Option<i64>,
    pub file_name: String,
    pub file_path: String,
    pub duration: Option<i64>,
    pub channels: Option<i64>,
    pub sample_rate: Option<i64>,
    pub bitrate: Option<i64>,
    pub file_size: Option<i64>,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct FileMetadata {
    pub id: i64,
    #[serde(flatten)]
    pub data: BaseFileMetadata,
}

pub type CreateFileMetadata = BaseFileMetadata;
