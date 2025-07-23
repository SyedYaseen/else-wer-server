use crate::db::audiobooks::insert_audiobook;
use crate::models::models::AudioBook;
use core::task;
use sqlx::SqlitePool;
use std::env;
use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::process;
use tokio::fs;
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

async fn has_dirs(path: &PathBuf) -> Result<bool, io::Error> {
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
) -> Result<(), Box<dyn Error + Send + Sync>> {
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

async fn process_files(
    book: &mut AudioBook,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
) -> Result<Vec<AudioBook>, Box<dyn std::error::Error + Send + Sync>> {
    let path = PathBuf::from(path_str);

    if !path.exists() {
        return Err("Provided path does not exist".into());
    }

    let mut audio_books: Vec<AudioBook> = Vec::new();
    let _ = recursive_dirscan(&path, &mut audio_books).await?;

    let mut tasks = vec![];
    for mut book in audio_books {
        let db = db.clone();

        tasks.push(tokio::spawn(async move {
            process_files(&mut book).await?;
            insert_audiobook(&db, &book).await?;

            Ok(book) as Result<AudioBook, Box<dyn std::error::Error + Send + Sync>>
        }));
    }

    let mut processed_books = Vec::new();
    for task in tasks {
        processed_books.push(task.await??);
    }

    Ok(processed_books)
    // Ok(())
}
