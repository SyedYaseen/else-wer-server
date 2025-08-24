use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolvedStatus {
    UnResolved = 0,
    AutoResolved = 1,
    UserResolved = 2,
    Ignored = 4,
}

impl ResolvedStatus {
    pub const fn value(&self) -> i64 {
        match self {
            ResolvedStatus::UnResolved => 0,
            ResolvedStatus::AutoResolved => 1,
            ResolvedStatus::UserResolved => 2,
            ResolvedStatus::Ignored => 3,
        }
    }

    pub const fn from_value(value: i64) -> Option<Self> {
        match value {
            0 => Some(ResolvedStatus::UnResolved),
            1 => Some(ResolvedStatus::AutoResolved),
            2 => Some(ResolvedStatus::UserResolved),
            3 => Some(ResolvedStatus::Ignored),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileScanCache {
    // pub library_id: i64,
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
    pub raw_metadata: Option<String>,
    pub hash: Option<String>,
    pub resolve_status: ResolvedStatus,
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
            raw_metadata: None,
            hash: None,
            resolve_status: ResolvedStatus::UnResolved,
        }
    }
}
