use crate::db::audiobooks::{get_audiobook_id, insert_audiobook, insert_file_metadata};
use crate::models::models::{AudioBook, CreateFileMetadata};
use anyhow::Ok;

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use sqlx::SqlitePool;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::result::Result;
use tokio::fs;
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
                    println!("Warn: Not a valid path during directory scan");
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

pub async fn extract_metadata(path: &OsStr) -> anyhow::Result<CreateFileMetadata> {
    let path = path.to_owned();
    let metadata = spawn_blocking(async || -> anyhow::Result<CreateFileMetadata> {
        let tagged_file = Probe::open(&path)?.read()?;

        let props = tagged_file.properties();
        let duration = Some(props.duration().as_secs());
        let channels = props.channels();
        let sample_rate = props.sample_rate();
        let bitrate = props.audio_bitrate();

        // let mut title = None;
        // let mut artist = None;
        // let mut album = None;

        // if let Some(tag) = tagged_file.primary_tag() {
        //     title = tag.get_string(&ItemKey::TrackTitle).map(|s| s.to_string());
        //     artist = tag
        //         .get_string(&ItemKey::AlbumArtist)
        //         .or_else(|| tag.get_string(&ItemKey::TrackArtist))
        //         .map(|s| s.to_string());
        //     album = tag.get_string(&ItemKey::AlbumTitle).map(|s| s.to_string());
        // };

        Ok(CreateFileMetadata::new(
            path,
            duration.map(|d| d as i64),
            channels.map(|c| c as i64),
            sample_rate.map(|sr| sr as i64),
            bitrate.map(|br| br as i64),
        ))
    })
    .await?;
    let metadata = metadata.await?;

    Ok(metadata)
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
                    "mp3" | "m4b" | "flac" => {
                        book.files.push(path.into_os_string());
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
                let mut metadata = extract_metadata(file).await?;
                metadata.book_id = bookid;

                match insert_file_metadata(&db, metadata).await {
                    Result::Ok(_) => (),
                    Err(_) => {
                        eprintln!(
                            "File: {} already mapped to bookid: {}",
                            file.to_str().unwrap_or_default(),
                            bookid
                        );
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
