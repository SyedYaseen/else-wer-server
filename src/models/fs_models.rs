use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileScanCache {
    // pub library_id: i64,
    pub author: Option<String>,
    pub title: Option<String>,
    pub file_path: String,
    pub file_name: String,
    pub series: Option<String>,
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
    pub raw_metadata: Option<String>,
    pub hash: Option<String>,
}

impl FileScanCache {
    pub fn new(file_path: String, file_name: String) -> FileScanCache {
        FileScanCache {
            file_path: file_path,
            file_name: file_name,
            duration: 0,
            file_size: 0,
            author: None,
            title: None,
            series: None,
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
            raw_metadata: None,
            hash: None,
        }
    }
}
