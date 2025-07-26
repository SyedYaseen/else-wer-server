use crate::db::audiobooks::{get_audiobook_id, insert_audiobook, insert_file_metadata};
use crate::models::audiobooks::{AudioBook, CreateFileMetadata};
use anyhow::{Ok, anyhow};

use lofty::file::AudioFile;
use lofty::probe::Probe;
use serde::Deserialize;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::result::Result;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;
use symphonia::default;
use tokio::fs;
use tokio::process::Command;
use tokio::task::spawn_blocking;

async fn has_dirs(path: &PathBuf) -> anyhow::Result<bool> {
    let mut entries = fs::read_dir(path).await?;
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_dir() {
            return Ok(true);
        }
    }
    return Ok(false);
}

async fn recursive_dirscan(path: &PathBuf, audio_books: &mut Vec<AudioBook>) -> anyhow::Result<()> {
    let mut entries = fs::read_dir(path).await?;

    while let Some(entry) = entries.next_entry().await? {
        let f_name = entry.file_name();
        let f_type = entry.file_type().await?;

        if f_type.is_dir() {
            let sub_dir = match f_name.to_str() {
                Some(f_name) => f_name,
                None => {
                    println!("Unable to decode utf-8 variable f_name");
                    continue;
                }
            };

            let mut sub_dir_path = PathBuf::from(path);
            sub_dir_path.push(sub_dir);

            if let Err(e) = Box::pin(recursive_dirscan(&sub_dir_path, audio_books)).await {
                eprintln!("Err reading folder {e}");
                continue;
            }

            let v: Vec<_> = sub_dir_path
                .components()
                .map(|c| c.as_os_str().to_str().unwrap_or("").to_string())
                .collect();

            let (author, series, title): (String, Option<String>, String) = match v.as_slice() {
                [_, author, series, title, ..] => (
                    author.to_string(),
                    Some(series.to_string()),
                    title.to_string(),
                ),
                [_, author, title, ..] => (author.to_string(), None, title.to_string()),
                _ => {
                    println!("Warn: Not a valid dir scan path");
                    continue;
                }
            };

            let is_series = series == None && has_dirs(&sub_dir_path).await?;
            if !is_series {
                let conent_path = match sub_dir_path.to_str() {
                    Some(s) => s.to_owned(),
                    None => {
                        eprintln!("Path is not valid UTF-8");
                        String::new()
                    }
                };
                audio_books.push(AudioBook::new(author, series, title, conent_path));
            }
        }
    }

    Ok(())
}

// Book exists: test - Book 5 Network effect
// This data/test/Book 5 Network effect/Network effect.m4b
// 2025-07-25T21:47:37.856978Z ERROR rustybookshelf::api::audiobooks: Error scanning files: No format could be determined from the provided file

//     This data/MarthaWells/The Murderbot Diaries/Book 4 Exit Strategy/Exit Strategy.m4b
// This data/MarthaWells/The Murderbot Diaries/Book 1 All Systems Red/All Systems Red.m4b

#[derive(Debug, Deserialize)]
struct FFProbeFormat {
    duration: Option<String>,
    bit_rate: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FFProbeData {
    format: FFProbeFormat,
}

pub async fn extract_metadata(path: &str) -> anyhow::Result<CreateFileMetadata> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            path,
        ])
        .output()
        .await
        .map_err(|e| anyhow!("Failed to run ffprobe: {e}"))?;

    if !output.status.success() {
        return Err(anyhow!("ffprobe failed for file '{}'", path));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let ff_data: FFProbeData = serde_json::from_str(&json_str)
        .map_err(|e| anyhow!("Failed to parse ffprobe JSON: {e}"))?;

    let duration = ff_data
        .format
        .duration
        .as_deref()
        .and_then(|d| d.parse::<f64>().ok())
        .map(|d| d as i64);

    let bitrate = ff_data
        .format
        .bit_rate
        .as_deref()
        .and_then(|b| b.parse::<i64>().ok());

    Ok(CreateFileMetadata::new(
        path.to_owned(),
        duration,
        None,
        None,
        bitrate,
    ))
}

async fn get_book_files(book: &mut AudioBook) -> anyhow::Result<()> {
    let mut entries = fs::read_dir(&book.content_path).await?;

    while let Some(entry) = entries.next_entry().await? {
        let f_type = entry.file_type().await?;

        if f_type.is_file() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                match ext.to_lowercase().as_str() {
                    "jpg" | "jpeg" | "png" => {
                        book.cover_art = Some(path.to_string_lossy().into_owned());
                    }
                    "mp3" | "m4b" | "flac" | "m4a" => {
                        book.files.push(path.to_string_lossy().into_owned());
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

pub async fn scan_for_audiobooks(
    path_str: &str,
    db: &SqlitePool,
) -> anyhow::Result<Vec<AudioBook>> {
    let path = PathBuf::from(path_str);

    if !path.exists() {
        println!("Attempting to create dir {}", path.display());
        if let Err(_) = fs::create_dir_all(&path).await {
            return Err(anyhow::anyhow!("Failed to create dir {}", path.display()));
        }
    }

    if !path.is_dir() {
        return Err(anyhow::anyhow!("'{}' is not a directory", path.display()));
    }

    let mut audio_books: Vec<AudioBook> = Vec::new();
    let _ = recursive_dirscan(&path, &mut audio_books).await?;

    let mut tasks = vec![];
    for mut book in audio_books {
        let db = db.clone();
        tasks.push(tokio::spawn(async move {
            get_book_files(&mut book).await?;

            let bookid = match insert_audiobook(&db, &book).await {
                Result::Ok(id) => id,
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("UNIQUE constraint failed") {
                        eprintln!("Book exists: {} - {}", book.author, book.title);
                        get_audiobook_id(&db, &book).await?
                    } else {
                        return Err(e.into());
                    }
                }
            };

            for file in &book.files {
                let mut metadata = extract_metadata(&file).await?;
                metadata.book_id = bookid;

                match insert_file_metadata(&db, metadata).await {
                    Result::Ok(_) => (),
                    Err(_) => {
                        eprintln!("File: {} already mapped to bookid: {}", file, bookid);
                    }
                };
            }
            Ok(book)
        }));
    }

    let mut processed_books = Vec::new();
    for task in tasks {
        processed_books.push(task.await??);
    }

    Ok(processed_books)
}
