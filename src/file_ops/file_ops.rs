use crate::db::audiobooks::insert_audiobook;
use crate::models::models::{AudioBook, CreateFileMetadata};
use anyhow::{Context, Ok};

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use sqlx::SqlitePool;
use std::ffi::OsStr;
use std::path::PathBuf;
use tokio::fs;
use tokio::task::spawn_blocking;
// async fn get_path() -> PathBuf {
//     let key = "AUDIOBOOKS_LOCATION";

//     match env::var(key) {
//         Ok(path) if !path.trim().is_empty() => {
//             let p = PathBuf::from(path);
//             if !p.exists() {
//                 if let Err(e) = fs::create_dir_all(&p) {
//                     eprintln!("Error: Failed to create directory {}. {}", p.display(), e);
//                     process::exit(1);
//                 }
//             }
//             p
//         }
//         _ => {
//             eprintln!("Env {key} doesn't exist. Setting default audiobooks location");
//             let default_path = PathBuf::from("data");

//             if !default_path.exists() {
//                 if let Err(e) = fs::create_dir_all(&default_path) {
//                     eprintln!(
//                         "Error: Failed to create default directory {}. {}",
//                         default_path.display(),
//                         e
//                     );
//                     process::exit(1);
//                 }
//             }
//             default_path
//         }
//     }
// }

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

pub fn extract_metadata(path: &OsStr) -> anyhow::Result<()> {
    // Read file with Lofty
    let tagged_file = Probe::open(path)?.read()?;

    let mut title = None;
    let mut artist = None;
    let mut album = None;
    let mut duration: Option<u64> = None;
    let mut bitrate: Option<u32> = None;

    if let Some(tag) = tagged_file.primary_tag() {
        title = tag.get_string(&ItemKey::TrackTitle).map(|s| s.to_string());
        artist = tag
            .get_string(&ItemKey::AlbumArtist)
            .or_else(|| tag.get_string(&ItemKey::TrackArtist))
            .map(|s| s.to_string());
        album = tag.get_string(&ItemKey::AlbumTitle).map(|s| s.to_string());
    };

    let props = tagged_file.properties();
    bitrate = props.audio_bitrate();
    duration = Some(props.duration().as_secs());

    println!(
        "{} {} {} {}",
        title.unwrap_or_else(|| "Titl not found".to_string()),
        artist.unwrap_or_else(|| "Artist not found".to_string()),
        duration.unwrap_or_default(), // album.unwrap_or_else(|| "Alb not found".to_string())
        bitrate.unwrap_or_default()
    );

    // Ok(())
    // Ok(BookMetadata {
    //     title,
    //     artist,
    //     album,
    //     duration_seconds: duration,
    // })
    Ok(())
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
        return Err(anyhow::anyhow!("Path '{}' doesn't exist", path.display()));
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
            // insert_audiobook(&db, &book).await?;

            for file in &book.files {
                let _ = extract_metadata(file);
            }
            Ok(book)
        }));
    }

    let mut processed_books = Vec::new();
    for task in tasks {
        processed_books.push(task.await??);
    }

    Ok(processed_books)
    // Ok(())
}
