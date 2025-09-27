#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::os::windows::fs::symlink_file;
use std::path::{Path, PathBuf};

use regex::Regex;
use sqlx::SqlitePool;
use tokio::fs;
use walkdir::WalkDir;

use crate::{
    api::api_error::ApiError,
    db::audiobooks::list_all_books,
    models::audiobooks::{AudioBook, AudioBookRow},
};

pub async fn create_cover_link(
    source: &Path,
    ext: &str,
    book: &AudioBookRow,
) -> Result<Option<String>, ApiError> {
    let cover_name = &book.title.replace(' ', "_").to_lowercase().to_owned();
    let re = Regex::new(r"[^a-z0-9_\-\.]").unwrap();
    let cover_name = re.replace_all(&cover_name, "");

    let link_name = format!("{}.{}", cover_name, ext);
    let link_path = std::env::current_dir()?.join("covers").join(&link_name);

    let source_path = std::env::current_dir()?.join(source);

    // println!("Creating symlink: {:?} -> {:?}", link_path, source_path);

    if !source_path.exists() {
        return Err(ApiError::IOErrCustom(format!(
            "Source does not exist: {:?}",
            source_path
        )));
    }

    if let Some(parent) = link_path.parent() {
        let _ = fs::create_dir_all(parent).await.map_err(|e| {
            tracing::error!("Err creating dir {}", parent.display());
            e
        });
    }

    #[cfg(unix)]
    {
        let _ = symlink(source_path, link_path).map_err(|e| {
            tracing::error!("Failed cover art symlink {}. {}", link_name, e.to_string());
        });
    }

    #[cfg(windows)]
    {
        // Windows only allows symlink creation with elevated privileges or dev mode
        if let Err(_) = symlink_file(source_path, target_path) {
            // fallback to copy
            fs::copy(source_path, target_path).map_err(|e| {
                tracing::error!("Failed to copy cover art {}. {}", link_name, e.to_string());
            });
        }
    }

    Ok(Some(format!("/covers/{}", link_name)))
}

pub async fn cover_links(db: &SqlitePool) -> Result<(), ApiError> {
    let books = list_all_books(db).await?;
    for book in books {
        for entry in WalkDir::new(&book.files_location)
            .contents_first(true)
            .max_depth(1)
        {
            match entry {
                Ok(file) => {
                    if file.file_type().is_file() {
                        let ext = file
                            .path()
                            .extension()
                            .and_then(|f| f.to_str())
                            .map(|f| f.to_lowercase());

                        if let Some(ext) = &ext {
                            if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp") {
                                let cover_art = create_cover_link(file.path(), ext, &book).await?;
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Err reading {:#?}", e)
                }
            }
        }
    }
    Ok(())
}
