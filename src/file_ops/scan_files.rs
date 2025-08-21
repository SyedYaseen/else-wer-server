use std::path::Path;
use walkdir::WalkDir;

use crate::{
    api::api_error::ApiError,
    file_ops::{utils::create_cover_link, word_cleanup::clean_metadata},
    models::fs_models::FileScanCache,
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

pub async fn extract_metadata(path: &Path, metadata: &mut FileScanCache) -> Result<(), ApiError> {
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
                            println!("track num {}", track);
                        }

                        if let Some(year) = tag.year() {
                            metadata.pub_year = Some(year as i64);
                            // println!("pub year {}", year);
                        }

                        if let Some(track_total) = tag.track_total() {
                            // metadata.pub_year = Some(year as i64);
                            println!("track total {}", track_total);
                        }
                    }

                    let properties = tagged_file.properties();

                    metadata.duration = properties.duration().as_millis() as i64;
                    metadata.bitrate = properties.audio_bitrate().map(|b| b as i64);
                }
                Err(e) => {
                    tracing::error!("Failed reading tagged file: {}", e);
                    return Err(ApiError::Internal("Failed reading tagged file".into()));
                }
            };
        } else {
            tracing::error!("Failed to guess file type {}", &metadata.file_path);
        }
    }

    Ok(())
}

pub async fn scan_files(path_str: &str, db: &SqlitePool) -> Result<(), ApiError> {
    for entry in WalkDir::new(path_str).contents_first(true) {
        if let Ok(item) = entry {
            if item.file_type().is_file() {
                let fpath = item.path();
                let ext = fpath
                    .extension()
                    .and_then(|f| f.to_str())
                    .map(|f| f.to_lowercase());

                let file_name = fpath
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                let path_owned = fpath.to_owned().to_string_lossy().to_string();

                let mut metadata = FileScanCache::new(path_owned, file_name.clone());
                if let Ok(f_meta) = fs::metadata(&metadata.file_path).await {
                    metadata.file_size = f_meta.len() as i64;
                }

                match ext {
                    Some(ext) if matches!(ext.as_str(), "mp3" | "m4b" | "flac" | "m4a") => {
                        extract_metadata(fpath, &mut metadata).await?;
                        println!("File {}", item.path().display());
                    }
                    Some(ext) if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp") => {
                        metadata.cover_art = create_cover_link(&fpath.to_path_buf(), &ext).await?;
                    }
                    Some(other_ext) => {
                        println!("Unsupported file type: {}", other_ext);
                    }
                    None => {
                        println!("File has no extension: {:?}", fpath);
                    }
                }
                println!("{:#?}", metadata);
            }
        }
    }
    // Once files are found, extract their metadata

    // capture the file name to look for clues on order of file

    // Cross referece the parents, grandparents to check for clues of series name or author name to verify
    Ok(())
}
