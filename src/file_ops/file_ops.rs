use crate::db::audiobooks::{
    get_audiobook_id, insert_audiobook, insert_file_metadata, update_audiobook_duration,
};
use crate::models::audiobooks::{AudioBook, CreateFileMetadata};
use anyhow::{Ok, anyhow};
use futures::future;
use lofty::file::AudioFile;
use lofty::probe::Probe;
use regex::Regex;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::result::Result;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::{fs, task};

#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::os::windows::fs::symlink_file;

async fn has_dirs(path: &PathBuf) -> anyhow::Result<bool> {
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
) -> anyhow::Result<()> {
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

            if let Err(e) = Box::pin(recursive_dirscan(
                &sub_dir_path,
                audio_books,
                last_path_component,
            ))
            .await
            {
                eprintln!("Err reading folder {e}");
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

            // println!("{:#?}", v);

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

pub async fn extract_metadata(path: &str) -> anyhow::Result<CreateFileMetadata> {
    let path_owned = path.to_owned();

    let metadata =
        task::spawn_blocking(
            move || match Probe::open(&path_owned).and_then(|p| p.read()) {
                Result::Ok(tagged_file) => {
                    let properties = tagged_file.properties();

                    let duration_ms = properties.duration().as_millis() as i64;
                    let bitrate = properties.audio_bitrate().map(|b| b as i64);

                    let file_name = Path::new(&path_owned)
                        .file_name()
                        .and_then(|os_str| os_str.to_str())
                        .unwrap_or_default()
                        .to_string();

                    Ok(CreateFileMetadata::new(
                        path_owned,
                        None,
                        file_name,
                        Some(duration_ms),
                        None,
                        None,
                        bitrate,
                    ))
                }
                Err(e) => {
                    eprintln!("{} {}", &path_owned, &e.to_string());
                    Err(anyhow!("Failed to read metadata for {}: {}", path_owned, e))
                }
            },
        )
        .await??;

    Ok(metadata)
}

pub async fn create_cover_link(
    source: &PathBuf,
    ext: &str,
    book: &mut AudioBook,
) -> anyhow::Result<()> {
    let cover_name = &book.title.replace(' ', "_").to_lowercase().to_owned();
    let re = Regex::new(r"[^a-z0-9_\-\.]").unwrap();
    let cover_name = re.replace_all(&cover_name, "");

    let link_name = format!("{}.{}", cover_name, ext);
    let link_path = std::env::current_dir()?.join("covers").join(&link_name);

    let source_path = std::env::current_dir()?.join(source);

    // println!("Creating symlink: {:?} -> {:?}", link_path, source_path);

    if !source_path.exists() {
        return Err(anyhow!("Source does not exist: {:?}", source_path));
    }

    if let Some(parent) = link_path.parent() {
        if let Err(e) = fs::create_dir_all(parent).await {
            println!("err creating dir {}", e);
        };
    }

    #[cfg(unix)]
    {
        if let Err(e) = symlink(source_path, link_path) {
            eprintln!("Err: {} while creating cover art link {}", e, link_name);
        };
    }

    #[cfg(windows)]
    {
        // Windows only allows symlink creation with elevated privileges or dev mode
        if let Err(_) = symlink_file(source_path, target_path) {
            // fallback to copy
            fs::copy(source_path, target_path)?;
        }
    }

    book.cover_art = Some(format!("/covers/{}", link_name));

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
                    "jpg" | "jpeg" | "png" | "webp" => {
                        create_cover_link(&path, ext, book).await?;
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
    let path: PathBuf = PathBuf::from(path_str);
    let last_path_component = path
        .iter()
        .last()
        .and_then(|s| s.to_str())
        .unwrap_or_default();

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
    let _ = recursive_dirscan(&path, &mut audio_books, last_path_component).await?;
    // println!("{:#?} {:#?}", audio_books, path);

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
                        eprintln!("{} by {} already exists in db", book.title, book.author);
                        get_audiobook_id(&db, &book).await?
                    } else {
                        eprintln!(
                            "Some other err when inserting book {} by {}",
                            book.author, book.title
                        );
                        return Err(e.into());
                    }
                }
            };

            let semaphore = Arc::new(Semaphore::new(4));

            let extract_tasks: Vec<_> = book
                .files
                .iter()
                .map(|file| {
                    let file_path = file.clone();
                    let permit = semaphore.clone().acquire_owned();
                    tokio::spawn(async move {
                        let _permit = permit.await?;
                        let mut metadata = extract_metadata(&file_path).await?;
                        metadata.book_id = bookid;
                        Ok(metadata)
                    })
                })
                .collect();

            let meta_files = future::try_join_all(extract_tasks).await?;

            // let metadata_files = meta_files.into_iter().filter_map(Result::ok);
            let mut meta_files = meta_files.into_iter().collect::<anyhow::Result<Vec<_>>>()?;

            meta_files.sort_by_key(|m| m.file_path.clone());

            let mut total_duration = 0;

            for (index, f) in meta_files.iter_mut().enumerate() {
                f.file_id = Some(index as i64 + 1);
                total_duration += match f.duration {
                    Some(v) => v,
                    None => 0,
                };
                insert_file_metadata(&db, f).await?
            }

            book.duration = total_duration;
            update_audiobook_duration(&db, bookid, &book).await?;
            Ok(book)
        }));
    }

    let mut processed_books = Vec::new();
    for task in tasks {
        processed_books.push(task.await??);
    }

    Ok(processed_books)
}
