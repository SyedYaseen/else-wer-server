use std::{
    collections::{HashSet, hash_set},
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

use crate::{
    api::api_error::ApiError,
    db::meta_scan::{add_modify_audiobook_files, fetch_stage_by_parent_path, save_stage_metadata},
    file_ops::{
        book_cover::cover_links,
        meta_cleanup::{grouped_meta_cleanup, meta_cleanup},
    },
    models::meta_scan::FileScanCache,
};

use lofty::{
    config::{ParseOptions, ParsingMode},
    file::{AudioFile, FileType, TaggedFileExt},
    probe::Probe,
    tag::{Accessor, Tag, TagType},
};

use sqlx::SqlitePool;
use tokio::fs;

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
                    metadata.title = Some(title.trim().to_string());
                }

                if let Some(artist) = tag.artist() {
                    metadata.author = Some(artist.trim().to_string());
                }

                if let Some(album) = tag.album() {
                    metadata.series = Some(album.trim().to_string());
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

pub async fn scan_files(path_str: &str, db: &SqlitePool) -> Result<i32, ApiError> {
    let mut count = 0;
    for entry in WalkDir::new(path_str).contents_first(true) {
        if let Ok(item) = entry {
            if item.file_type().is_file() {
                let fpath = item.path();

                match fpath
                    .extension()
                    .and_then(|f| f.to_str())
                    .map(|f| f.to_lowercase())
                {
                    Some(ext) if matches!(ext.as_str(), "mp3" | "m4b" | "flac" | "m4a") => ext,
                    _ => continue, // Skip if no valid extension
                };

                let mut metadata = create_metadata(&fpath).await;

                if let Err(e) = extract_metadata(&mut metadata).await {
                    tracing::error!("Failed to extract metadata {} | {}.", fpath.display(), e);
                }

                meta_cleanup(&mut metadata);
                if let Err(e) = save_stage_metadata(db, metadata).await {
                    tracing::error!("Failed to save {}", e);
                }

                if let Err(e) = add_modify_audiobook_files(db).await {
                    tracing::error!("Failed to update row on server database {}", e);
                }
                count += 1;
            }
        }
    }

    let _ = cover_links(db).await.inspect_err(|e| {
        tracing::error!("Failed to get cover for {}", e);
    });
    // Second db scan and cleanup
    // grouped_meta_cleanup(db).await;

    // capture the file name to look for clues on order of file

    // Cross referece the parents, grandparents to check for clues of series name or author name to verify
    Ok(count)
}

pub async fn testscan_files(path_str: &str, db: &SqlitePool) -> Result<i32, ApiError> {
    let mut count = 0;
    let mut parent_path: Option<PathBuf> = None;
    let mut scan_cache: HashSet<String> = vec![].into_iter().collect();
    for entry in WalkDir::new(path_str).contents_first(true) {
        if let Ok(item) = entry {
            if item.file_type().is_file() {
                let fpath = item.path();

                // Skip if unsupported ftype
                match fpath
                    .extension()
                    .and_then(|f| f.to_str())
                    .map(|f| f.to_lowercase())
                {
                    Some(ext) if matches!(ext.as_str(), "mp3" | "m4b" | "flac" | "m4a") => ext,
                    _ => continue, // Skip if no valid extension
                };

                // Skip if already scanned
                if let Some(parent) = fpath.parent() {
                    if parent_path.as_deref() != Some(parent) {
                        parent_path = Some(parent.to_path_buf());
                        scan_cache = fetch_stage_by_parent_path(db, parent).await?;
                    }
                }

                if let Some(fpath_str) = fpath.to_str() {
                    if scan_cache.contains(fpath_str) {
                        continue;
                    }
                } else {
                    continue; // skip non-UTF8 paths
                }

                // Proceed, if previously unscanned.
                let mut metadata = create_metadata(&fpath).await;
                println!("{:#?}", metadata);
                if let Err(e) = extract_metadata(&mut metadata).await {
                    tracing::error!("Failed to extract metadata {} | {}.", fpath.display(), e);
                }

                meta_cleanup(&mut metadata);
                if let Err(e) = save_stage_metadata(db, metadata).await {
                    tracing::error!("Failed to save {}", e);
                }

                if let Err(e) = add_modify_audiobook_files(db).await {
                    tracing::error!("Failed to update row on server database {}", e);
                }
                count += 1;
            }
        }
    }

    let _ = cover_links(db).await.inspect_err(|e| {
        tracing::error!("Failed to get cover for {}", e);
    });
    Ok(count)
}
