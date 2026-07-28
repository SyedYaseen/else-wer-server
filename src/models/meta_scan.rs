use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

/// One in-memory scan record for a single file, built by `scan_files` and consumed
/// directly by `group_and_attach_files` (no DB staging table since 0008 dropped
/// `file_scan_cache`).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FileScanCache {
    pub author: Option<String>,
    pub title: Option<String>,
    pub clean_title: Option<String>,
    pub file_path: String,
    pub path_parent: String,
    pub file_name: String,
    pub series: Option<String>,
    pub dramatized: bool,
    pub clean_series: Option<String>,
    pub series_part: Option<i64>,
    pub cover_art: Option<String>,
    pub pub_year: Option<i64>,
    pub narrated_by: Option<String>,
    pub duration: i64,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
    pub file_size: i64,
    pub mime_type: Option<String>,
    pub channels: Option<i64>,
    pub sample_rate: Option<i64>,
    pub bitrate: Option<i64>,
    pub extracts: Option<String>,
}

impl FileScanCache {
    pub fn new(file_path: String, file_name: String, path_parent: String) -> FileScanCache {
        FileScanCache {
            file_path: file_path,
            file_name: file_name,
            path_parent: path_parent,
            dramatized: false,
            duration: 0,
            file_size: 0,
            author: None,
            title: None,
            clean_title: None,
            series: None,
            clean_series: None,
            series_part: None,
            cover_art: None,
            pub_year: None,
            narrated_by: None,
            track_number: None,
            disc_number: None,
            mime_type: None,
            channels: None,
            sample_rate: None,
            bitrate: None,
            extracts: None,
        }
    }
}

#[derive(Serialize, Debug, FromRow)]
pub struct FileInfo {
    pub id: i64,
    pub book_id: i64,
    pub author: String,
    pub title: String,
    pub series: String,
    pub file_path: String,
    pub path_parent: String,
    pub file_name: String,
}

#[derive(Serialize)]
pub struct BookInfo {
    pub series: String,
    pub files: Vec<FileInfo>,
}

#[derive(Serialize)]
pub struct AuthorInfo {
    pub books: Vec<BookInfo>,
}

#[derive(Serialize)]
pub struct FileScanGrouped {
    pub series: String,
    pub authors: Vec<AuthorInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChangeType {
    Rename,
    MoveTitle,
    MergeTitle,
    FileMove,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeDto {
    pub change_type: ChangeType,

    pub file_ids: Vec<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_book_ids: Option<Vec<i64>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_book_id: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_author: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_series: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_filetitle: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_author: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_series: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_filetitle: Option<String>,
}
