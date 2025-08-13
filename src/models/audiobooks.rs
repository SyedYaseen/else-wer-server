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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AudioBook {
    pub author: String,
    pub series: Option<String>,
    pub title: String,
    pub content_path: String,
    pub cover_art: Option<String>,
    pub duration: i64,
    pub metadata: Option<String>,
    pub files: Vec<String>,
}

impl AudioBook {
    pub fn new(
        author: String,
        series: Option<String>,
        title: String,
        content_path: String,
    ) -> AudioBook {
        AudioBook {
            author: author,
            series: series,
            title: title,
            content_path: content_path,
            cover_art: None,
            duration: 0,
            metadata: None,
            files: Vec::new(),
        }
    }
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
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct FileMetadata {
    pub id: i64,
    #[serde(flatten)]
    pub data: BaseFileMetadata,
}

pub type CreateFileMetadata = BaseFileMetadata;

impl CreateFileMetadata {
    pub fn new(
        file_path: String,
        file_id: Option<i64>,
        file_name: String,
        duration: Option<i64>,
        channels: Option<i64>,
        sample_rate: Option<i64>,
        bitrate: Option<i64>,
    ) -> CreateFileMetadata {
        CreateFileMetadata {
            book_id: -99,
            file_id: file_id,
            file_name: file_name,
            file_path: file_path,
            duration: duration,
            channels: channels,
            sample_rate: sample_rate,
            bitrate: bitrate,
        }
    }
}
