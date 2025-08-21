use std::path::PathBuf;

use regex::Regex;
use tokio::fs;

use crate::api::api_error::ApiError;

pub async fn create_cover_link(source: &PathBuf, ext: &str) -> Result<Option<String>, ApiError> {
    // let cover_name = &book.title.replace(' ', "_").to_lowercase().to_owned();
    let cover_name = "dummyname";
    let re = Regex::new(r"[^a-z0-9_\-\.]").unwrap();
    let cover_name = re.replace_all(&cover_name, "");

    let link_name = format!("{}.{}", cover_name, ext);
    let link_path = std::env::current_dir()?.join("covers").join(&link_name);

    let source_path = std::env::current_dir()?.join(source);

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
        use tokio::fs::symlink;

        let _ = symlink(source_path, link_path).await?;
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
