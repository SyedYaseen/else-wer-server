use crate::api::api_error::ApiError;
use crate::db::audiobooks::{
    insert_audiobook, insert_file_metadata, list_all_books, update_audiobook_duration,
};
use crate::models::audiobooks::{AudioBook, AudioBookRow, CreateFileMetadata};
use futures::{StreamExt, stream};

use lofty::file::FileType;
use lofty::properties;
use lofty::{
    config::{ParseOptions, ParsingMode},
    file::AudioFile,
    probe::Probe,
};
use regex::Regex;
use sqlx::{Pool, Sqlite, SqlitePool};
use std::path::{Path, PathBuf};
use std::result::Result;
use symphonia::core::meta;
use tokio::{fs, task::JoinHandle};
use tracing::{info, warn};

#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::os::windows::fs::symlink_file;

async fn has_dirs(path: &PathBuf) -> Result<bool, ApiError> {
    let mut entries = fs::read_dir(path).await?;
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_dir() {
            return Ok(true);
        }
    }
    return Ok(false);
}

async fn recursive_dirscan(
    path: &PathBuf,
    audio_books: &mut Vec<AudioBook>,
    last_path_component: &str,
) -> Result<(), ApiError> {
    let mut entries = fs::read_dir(path).await?;

    while let Some(entry) = entries.next_entry().await? {
        let f_name = entry.file_name();
        let f_type = entry.file_type().await?;

        if !f_type.is_dir() {
            continue;
        }

        let Some(sub_dir) = f_name.to_str() else {
            warn!("Unable to decode utf-8 variable f_name");
            continue;
        };

        let sub_dir_path = PathBuf::from(path).join(sub_dir);

        if let Err(e) = Box::pin(recursive_dirscan(
            &sub_dir_path,
            audio_books,
            last_path_component,
        ))
        .await
        {
            tracing::error!("Err reading folder {e}");
            continue;
        }

        let mut v: Vec<_> = sub_dir_path
            .components()
            .map(|c| c.as_os_str().to_str().unwrap_or("").to_string())
            .collect();

        if sub_dir_path.is_absolute() {
            if let Some(index) = &v.iter().position(|c| c == last_path_component) {
                v.drain(..index);
            }
        }

        let (author, series, title): (String, Option<String>, String) = match v.as_slice() {
            [_, author, series, title, ..] => (
                author.to_string(),
                Some(series.to_string()),
                title.to_string(),
            ),
            [_, author, title, ..] => (author.to_string(), None, title.to_string()),
            _ => {
                info!("Skipping invalid path");
                continue;
            }
        };

        let is_series = series == None && has_dirs(&sub_dir_path).await?;
        if !is_series {
            let Some(conent_path) = sub_dir_path.to_str().to_owned() else {
                warn!("Path is not valid UTF-8");
                continue;
            };
            audio_books.push(AudioBook::new(
                author,
                series,
                title,
                conent_path.to_string(),
            ));
        }
    }

    Ok(())
}

pub async fn create_cover_link(
    source: &PathBuf,
    ext: &str,
    book: &mut AudioBook,
) -> Result<(), ApiError> {
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

    book.cover_art = Some(format!("/covers/{}", link_name));

    Ok(())
}

async fn get_book_files_link_covers(book: &mut AudioBook) -> Result<(), ApiError> {
    let mut entries = fs::read_dir(&book.content_path).await?;
    // TODO debug here for missing files
    while let Some(entry) = entries.next_entry().await? {
        let f_type = entry.file_type().await?;
        if !f_type.is_file() {
            continue;
        }

        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            match ext.to_lowercase().as_str() {
                "jpg" | "jpeg" | "png" | "webp" => {
                    create_cover_link(&path, ext, book).await?;
                }
                "mp3" | "m4b" | "flac" | "m4a" => {
                    book.files.push(path.to_string_lossy().into_owned());
                }
                _ => {}
            }
        } else {
            info!("Ext not found: {}", path.display());
        }
    }

    Ok(())
}

async fn capture_files_cover_paths(
    audio_books: Vec<AudioBook>,
    db: &Pool<Sqlite>,
) -> Vec<(i64, AudioBook)> {
    let mut insert_tasks: Vec<JoinHandle<Result<(i64, AudioBook), ApiError>>> = vec![];

    for mut book in audio_books {
        info!("==== Before extracting, {}", book.title);
        let db = db.clone();
        insert_tasks.push(tokio::spawn(async move {
            if let Err(e) = get_book_files_link_covers(&mut book).await {
                tracing::error!("{} {}", book.title, e.to_string());
            }
            let book_id = insert_audiobook(&db, &book).await?;

            Ok((book_id, book))
        }));
    }
    let mut processed_books: Vec<(i64, AudioBook)> = Vec::new();

    // await all tasks
    for task in insert_tasks {
        match task.await {
            Ok(Ok((book_id, book))) => {
                processed_books.push((book_id, book));
            }
            Ok(Err(e)) => {
                tracing::error!("Task failed: {:?}", e);
            }
            Err(join_err) => {
                tracing::error!("Task panicked: {:?}", join_err);
            }
        }
    }

    processed_books
}

pub async fn extract_metadata(path: &str) -> Result<CreateFileMetadata, ApiError> {
    let path_owned = path.trim().to_owned();

    let file_name = Path::new(&path_owned)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    let mut metadata =
        CreateFileMetadata::new(path_owned.clone(), None, file_name, None, None, None, None);

    let probe = Probe::open(&path_owned).inspect_err(|e| {
        tracing::error!(
            "Failed to create metadata probe {path_owned} {}",
            e.to_string()
        );
    });

    if let Ok(probe) = probe {
        let probe = probe.options(ParseOptions::new().parsing_mode(ParsingMode::Relaxed));

        if let Ok(probe) = probe.guess_file_type() {
            match probe.read() {
                Ok(tagged_file) => {
                    let properties = tagged_file.properties();
                    metadata.duration = Some(properties.duration().as_millis() as i64);
                    metadata.bitrate = properties.audio_bitrate().map(|b| b as i64);
                }
                Err(e) => {
                    tracing::error!("Failed reading tagged file: {}", e);
                    return Err(ApiError::Internal("Failed reading tagged file".into()));
                }
            };
        } else {
            tracing::error!("Failed to guess file type {}", path_owned);
        }
    }

    Ok(metadata)
}

async fn capture_metadata(
    audio_books: Vec<(i64, AudioBook)>,
    db: &SqlitePool,
) -> Result<(), ApiError> {
    stream::iter(audio_books)
        .map(|(book_id, book)| async move {
            let files = book.files.clone();

            let mut metadata: Vec<CreateFileMetadata> = stream::iter(files)
                .map(|file| async move {
                    info!("Meta Extract {file}");

                    match extract_metadata(&file).await {
                        Ok(mut metadata) => {
                            metadata.book_id = book_id;
                            Some(metadata)
                        }
                        Err(_) => {
                            tracing::error!("Error getting metadata for {}", file);
                            None
                        }
                    }
                })
                .buffer_unordered(5)
                .filter_map(|m| async move { m })
                .collect()
                .await;

            metadata.sort_by_key(|m| m.file_name.clone());
            let mut total_duration = 0;

            for (index, f) in metadata.iter_mut().enumerate() {
                f.file_id = Some(index as i64 + 1);
                total_duration += f.duration.unwrap_or(0);
                insert_file_metadata(&db, f)
                    .await
                    .inspect_err(|e| {
                        tracing::error!("Err inserting {} metadata, Err: {}", f.file_name, e)
                    })
                    .ok();
            }

            update_audiobook_duration(&db, book_id.to_owned(), total_duration)
                .await
                .inspect_err(|e| tracing::error!("Err updating duration {}. {}", book.title, e))
                .ok();

            metadata
        })
        .buffer_unordered(2)
        .collect::<Vec<_>>()
        .await;

    Ok(())
}

pub async fn scan_for_audiobooks(
    path_str: &str,
    db: &SqlitePool,
) -> Result<Vec<AudioBookRow>, ApiError> {
    let path: PathBuf = PathBuf::from(path_str);

    if !path.exists() {
        info!("Attempting to create dir {}", path.display());
        fs::create_dir_all(&path).await?;
    }

    if !path.is_dir() {
        return Err(ApiError::IOErrCustom(format!(
            "'{}' is not a directory",
            path.display()
        )));
    }
    let last_path_component = path
        .iter()
        .last()
        .and_then(|s| s.to_str())
        .ok_or_else(|| ApiError::Internal("Invalid audiobook path".into()))?;

    let mut audio_books: Vec<AudioBook> = Vec::new();
    let _ = recursive_dirscan(&path, &mut audio_books, last_path_component).await?;
    let inserted_books = capture_files_cover_paths(audio_books, &db).await;

    capture_metadata(inserted_books, &db).await?;

    let audio_books = list_all_books(db).await?;

    Ok(audio_books)
}
