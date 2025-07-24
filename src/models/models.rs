use std::ffi::OsString;

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
#[derive(Debug, serde::Serialize)]
pub struct AudioBook {
    pub author: String,
    pub series: Option<String>,
    pub title: String,
    pub content_path: String,
    pub cover_art: Option<String>,
    pub metadata: Option<String>,
    pub files: Vec<OsString>,
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
            metadata: None,
            files: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BaseFileMetadata {
    pub book_id: i32,
    pub file_path: String,
    pub codec: Option<String>,
    pub duration: Option<i64>,
    pub channels: Option<i64>,
    pub sample_rate: Option<i64>,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct FileMetadata {
    pub id: i64,
    #[serde(flatten)]
    pub data: BaseFileMetadata,
}

pub type CreateFileMetadata = BaseFileMetadata;

#[derive(Debug)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub password_hash: String,
    pub salt: String,
}

#[derive(Debug)]
pub struct Progress {
    pub id: i32,
    pub user_id: i32,
    pub book_id: i32,
    pub progress_fname: Option<String>,
    pub progress_time_marker: i32,
}
