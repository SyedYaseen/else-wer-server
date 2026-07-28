#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::os::windows::fs::symlink_file;
use std::path::Path;

use regex::Regex;
use sqlx::SqlitePool;
use tokio::fs;
use walkdir::WalkDir;

use crate::{
    api::api_error::ApiError,
    db::audiobooks::{list_books, update_cover_art},
    models::audiobooks::AudioBookRow,
};

pub async fn create_cover_link(
    source: &Path,
    ext: &str,
    book: &AudioBookRow,
) -> Result<Option<String>, ApiError> {
    let source_path = std::env::current_dir()?.join(source);

    let source_meta = fs::metadata(&source_path).await.map_err(|_| {
        ApiError::IOErrCustom(format!("Source does not exist: {:?}", source_path))
    })?;
    // Cache-bust key: derived from the source file's mtime so the URL only
    // changes when the underlying image content actually does (e.g. a
    // replaced cover), and repeated rescans of an unchanged file are stable.
    let version = source_meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let cover_name = &book.title.replace(' ', "_").to_lowercase().to_owned();
    let re = Regex::new(r"[^a-z0-9_\-\.]").unwrap();
    let cover_name = re.replace_all(&cover_name, "");

    let link_name = format!("{}.{}", cover_name, ext);
    let link_path = std::env::current_dir()?.join("covers").join(&link_name);
    let link_url = format!("/covers/{}?v={}", link_name, version);

    // Only touch the symlink if it's missing, dangling, or points at a
    // different file — covers.exists() alone can't tell those apart from
    // "already correct", which previously left stale/wrong links in place.
    let up_to_date = fs::read_link(&link_path)
        .await
        .is_ok_and(|target| target == source_path);

    if up_to_date {
        return Ok(Some(link_url));
    }
    let _ = fs::remove_file(&link_path).await;

    if let Some(parent) = link_path.parent() {
        let _ = fs::create_dir_all(parent).await.map_err(|e| {
            tracing::error!("Err creating dir {}", parent.display());
            e
        });
    }

    #[cfg(unix)]
    {
        let _ = symlink(&source_path, &link_path).map_err(|e| {
            tracing::error!("Failed cover art symlink {}. {}", link_name, e.to_string());
        });
    }

    #[cfg(windows)]
    {
        // Windows only allows symlink creation with elevated privileges or dev mode
        if symlink_file(&source_path, &link_path).is_err() {
            // fallback to copy
            if let Err(e) = fs::copy(&source_path, &link_path).await {
                tracing::error!("Failed to copy cover art {}. {}", link_name, e.to_string());
            }
        }
    }

    Ok(Some(link_url))
}

/// Download a candidate's cover image and link it in like any other book cover.
/// Best-effort: callers should treat errors as non-fatal to the calling request.
pub async fn download_cover(url: &str, book: &AudioBookRow) -> Result<Option<String>, ApiError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    let bytes = client.get(url).send().await?.error_for_status()?.bytes().await?;

    let ext = url
        .split('?')
        .next()
        .unwrap_or(url)
        .rsplit('.')
        .next()
        .filter(|e| matches!(e.to_lowercase().as_str(), "jpg" | "jpeg" | "png" | "webp"))
        .unwrap_or("jpg")
        .to_lowercase();

    let dest = Path::new(&book.files_location).join(format!("cover.{ext}"));
    fs::write(&dest, &bytes)
        .await
        .map_err(|e| ApiError::IOErrCustom(e.to_string()))?;

    create_cover_link(&dest, &ext, book).await
}

pub async fn cover_links(db: &SqlitePool) -> Result<(), ApiError> {
    let books = list_books(db, None, None).await?;
    for book in books {
        for entry in WalkDir::new(&book.files_location)
            .contents_first(true)
            .max_depth(2)
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
                                if let Some(cover_link) = cover_art {
                                    update_cover_art(&db, book.id, cover_link).await?;
                                }
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
