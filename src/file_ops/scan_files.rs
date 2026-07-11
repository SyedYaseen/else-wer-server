use axum::extract::path;
use futures::{StreamExt, stream};
use std::{
    fs::File,
    io::BufReader,
    mem::take,
    path::{Path, PathBuf},
    sync::Arc,
};
use walkdir::WalkDir;

use crate::{
    api::api_error::ApiError,
    db::libraries::list_libraries,
    db::meta_scan::{delete_removed_files, fetch_known_file_paths, group_and_attach_files},
    file_ops::meta_cleanup::meta_cleanup,
    models::meta_scan::FileScanCache,
};

use lofty::{
    config::{ParseOptions, ParsingMode},
    file::{AudioFile, FileType, TaggedFileExt},
    probe::Probe,
    tag::{Accessor, Tag, TagType},
};

use sqlx::SqlitePool;
use tokio::{fs, sync::RwLock};

pub async fn extract_besttag(tags: &[Tag]) -> Option<&Tag> {
    let priority = [
        TagType::Id3v2,
        TagType::VorbisComments,
        TagType::Mp4Ilst,
        TagType::Ape,
        TagType::Id3v1,
    ];

    for tag_type in &priority {
        if let Some(tag) = tags.iter().find(|t| t.tag_type() == *tag_type) {
            let title = tag.title().unwrap_or_default().to_string();
            let artist = tag.artist().unwrap_or_default().to_string();

            if !title.is_empty() && !artist.is_empty() {
                return Some(tag);
            }
        }
    }
    tags.first()
}

fn get_mime_type(file_type: &Option<FileType>) -> Option<String> {
    if let Some(file_type) = file_type {
        match file_type {
            FileType::Mpeg => Some("audio/mpeg".to_string()),
            FileType::Mp4 => Some("audio/mp4".to_string()),
            FileType::Flac => Some("audio/flac".to_string()),
            FileType::Wav => Some("audio/wav".to_string()),
            FileType::Mpc => Some("audio/x-musepack".to_string()),
            FileType::Aiff => Some("audio/aiff".to_string()),
            FileType::Ape => Some("audio/ape".to_string()),
            FileType::Aac => Some("audio/aac".to_string()),
            FileType::Opus => Some("audio/opus".to_string()),
            FileType::Vorbis => Some("audio/vorbis".to_string()),
            FileType::Speex => Some("audio/speex".to_string()),
            FileType::WavPack => Some("audio/wavpack".to_string()),
            _ => None, // unsupported types
        }
    } else {
        None
    }
}

async fn extract_tag(
    probe: Probe<BufReader<File>>,
    metadata: &mut FileScanCache,
) -> Result<(), ApiError> {
    match probe.read() {
        Ok(tagged_file) => {
            if let Some(tag) = extract_besttag(tagged_file.tags()).await {
                if let Some(title) = tag.title() {
                    metadata.title = Some(title.trim().to_lowercase().to_string());
                }

                if let Some(artist) = tag.artist() {
                    metadata.author = Some(artist.trim().to_lowercase().to_string());
                } else {
                    metadata.author = Some("unknown".to_string());
                }

                if let Some(album) = tag.album() {
                    metadata.series = Some(album.trim().to_lowercase().to_string());
                }

                if let Some(track) = tag.track() {
                    metadata.track_number = Some(track as i64);
                }

                if let Some(year) = tag.year() {
                    metadata.pub_year = Some(year as i64);
                    // println!("pub year {}", year);
                }

                // if let Some(track_total) = tag.track_total() {
                //     println!("track total {}", track_total);
                // }
            }

            let properties = tagged_file.properties();

            metadata.duration = properties.duration().as_millis() as i64;
            metadata.bitrate = properties.audio_bitrate().map(|b| b as i64);
        }
        Err(e) => {
            tracing::error!("Failed reading tagged file: {}", e);
            // return Err(ApiError::Internal("Failed reading tagged file".into()));
        }
    };
    Ok(())
}

pub async fn extract_metadata(metadata: &mut FileScanCache) -> Result<(), ApiError> {
    let probe = Probe::open(&metadata.file_path).inspect_err(|e| {
        tracing::error!(
            "Failed to create metadata probe {} {}",
            &metadata.file_path,
            e.to_string()
        );
    });

    if let Ok(probe) = probe {
        let probe = probe.options(ParseOptions::new().parsing_mode(ParsingMode::Relaxed));

        if let Ok(probe) = probe.guess_file_type() {
            metadata.mime_type = get_mime_type(&probe.file_type());
            let _ = extract_tag(probe, metadata).await;
        } else {
            tracing::error!("Failed to guess file type {}", &metadata.file_path);
        }
    }

    Ok(())
}

async fn create_metadata(fpath: &Path) -> FileScanCache {
    let file_name = fpath
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let path_parent = match fpath.parent() {
        Some(p) => p.to_string_lossy().to_string(),
        _ => "".to_owned(),
    };

    let path_owned = fpath.to_string_lossy().to_string();

    let mut metadata = FileScanCache::new(path_owned, file_name, path_parent);
    if let Ok(f_meta) = fs::metadata(&metadata.file_path).await {
        metadata.file_size = f_meta.len() as i64;
    }
    metadata
}

async fn capture_file_paths(path_str: &str) -> Vec<PathBuf> {
    WalkDir::new(path_str)
        .contents_first(true)
        .into_iter()
        .filter_map(|f| f.ok())
        .filter(|f| f.file_type().is_file())
        .map(|f| f.path().to_owned())
        .collect()
}

pub async fn scan_files(library_id: i64, path_str: &str, db: &SqlitePool) -> Result<u64, ApiError> {
    tracing::info!("Scanning audiobook location: {path_str}");

    // WalkDir silently yields zero entries for a missing/unreadable path, which
    // would otherwise look identical to "every file in this library was deleted"
    // below and wipe the library's catalog. Fail loudly instead of treating an
    // unreachable path (typo'd edit, unmounted disk) as a mass deletion.
    if !Path::new(path_str).is_dir() {
        return Err(ApiError::BadRequest(format!(
            "Library path is not a reachable directory: {path_str}"
        )));
    }

    let scan_cache = Arc::new(RwLock::new(fetch_known_file_paths(db, library_id).await?));
    let fsc_metadatas: Arc<RwLock<Vec<FileScanCache>>> = Arc::new(RwLock::new(Vec::new()));
    let paths = capture_file_paths(path_str).await;

    stream::iter(paths)
        .map(|p| {
            let cache = Arc::clone(&scan_cache);
            let metadata_list = Arc::clone(&fsc_metadatas);
            tokio::spawn(async move {
                let ext = p
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|ext| ext.to_lowercase());

                if !matches!(ext.as_deref(), Some("mp3" | "m4b" | "flac" | "m4a")) {
                    return Ok(0);
                }

                if let Some(fp) = p.to_str() {
                    let read_cache = cache.read().await;
                    if read_cache.contains_key(fp) {
                        drop(read_cache);
                        let mut write_cache = cache.write().await;
                        write_cache.remove(fp);
                        return Ok(0);
                    }
                }

                let mut metadata = create_metadata(&p).await;

                if let Err(e) = extract_metadata(&mut metadata).await {
                    tracing::error!("Failed to extract metadata {} | {}.", p.display(), e);
                }

                meta_cleanup(&mut metadata);

                let mut metadata_write = metadata_list.write().await;
                metadata_write.push(metadata.clone());

                Ok::<u32, ApiError>(1)
            })
        })
        .buffer_unordered(10) // limits to 10 concurrent tasks
        .collect::<Vec<_>>()
        .await;

    let paths_to_delete: Vec<i64> = {
        let cache = scan_cache.read().await;
        cache.values().cloned().collect()
    };

    if !paths_to_delete.is_empty() {
        delete_removed_files(db, &paths_to_delete).await?;
    }

    let metadatas = {
        let mut guard = fsc_metadatas.write().await;
        let metadatas = take(&mut *guard);
        drop(guard);
        tracing::info!("Files to attach: {}", metadatas.len());
        metadatas
    };

    const CHUNK_SIZE: usize = 500;

    let mut count = 0;
    if !metadatas.is_empty() {
        for chunk in metadatas.chunks(CHUNK_SIZE) {
            count += group_and_attach_files(db, library_id, chunk).await?;
        }
    }

    Ok(count)
}

/// Scans every configured library root in turn, summing the new-files count.
/// Callers (manual/upload/lazy scan triggers) already serialize against each other
/// via `AppState::scan_guard`; this just replaces the old single hardcoded-path call.
///
/// Each library's scan is independent: one library failing (e.g. an unmounted disk)
/// is logged and skipped rather than aborting the whole batch — libraries scanned
/// before the failure keep their committed results, and libraries after it still get
/// their turn, unlike propagating the first error via `?` which would silently drop
/// everything already accomplished and skip everything still to come.
pub async fn scan_all_libraries(db: &SqlitePool) -> Result<u64, ApiError> {
    let libraries = list_libraries(db).await?;
    let mut count = 0;
    for library in libraries {
        match scan_files(library.id, &library.path, db).await {
            Ok(n) => count += n,
            Err(e) => tracing::error!(
                library_id = library.id,
                library_path = %library.path,
                "scan failed for library, continuing with remaining libraries: {e}"
            ),
        }
    }
    Ok(count)
}
