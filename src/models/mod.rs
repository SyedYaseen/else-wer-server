use std::{ffi::OsString, path::PathBuf};

#[derive(Debug)]
pub struct AudioBook {
    pub author: String,
    pub series: Option<String>,
    pub title: String,
    pub content_path: OsString,
    pub cover_art: Option<String>,
    pub metadata: Option<String>,
    pub files: Vec<OsString>
}

impl AudioBook {
    pub fn new(author: String, series: Option<String>, title: String, content_path: OsString ) -> AudioBook {
        AudioBook {
            author: author,
            series: series,
            title: title,
            content_path: content_path,
            cover_art: None,
            metadata: None,
            files: Vec::new()
        }
    }
}

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
