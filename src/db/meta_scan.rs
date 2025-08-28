use std::{collections::HashMap, hash::Hash};

use axum::extract::{Path, path};
use sqlx::{Pool, Sqlite};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};

use crate::{
    api::api_error::ApiError,
    models::meta_scan::{AuthorInfo, FileInfo, FileScanCache, FileScanGrouped},
};

pub async fn save_meta(db: &Pool<Sqlite>, metadata: FileScanCache) -> Result<(), ApiError> {
    let resolve_status = metadata.resolve_status.value();
    let rawmet = "".to_owned();
    let save_res = sqlx::query!(
        r#"
            INSERT INTO file_scan_cache (
                author, title, clean_title, file_path, file_name, path_parent, series, clean_series, series_part, 
                cover_art, pub_year, narrated_by, duration, track_number, 
                disc_number, file_size, mime_type, channels, sample_rate, 
                bitrate, dramatized, extracts,  raw_metadata, resolve_status, hash
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, 
                $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25
            )
            ON CONFLICT(file_path) DO UPDATE SET
                author = excluded.author,
                title = excluded.title,
                clean_title = excluded.clean_title,
                file_name = excluded.file_name,
                path_parent = excluded.path_parent,
                clean_series = excluded.clean_series,
                clean_title = excluded.clean_title,
                series_part = excluded.series_part,
                cover_art = excluded.cover_art,
                pub_year = excluded.pub_year,
                narrated_by = excluded.narrated_by,
                duration = excluded.duration,
                track_number = excluded.track_number,
                disc_number = excluded.disc_number,
                file_size = excluded.file_size,
                mime_type = excluded.mime_type,
                channels = excluded.channels,
                sample_rate = excluded.sample_rate,
                bitrate = excluded.bitrate,
                dramatized = excluded.dramatized,
                extracts = excluded.extracts,
                raw_metadata = excluded.raw_metadata,
                resolve_status = excluded.resolve_status,
                hash = excluded.hash,
                updated_at = CURRENT_TIMESTAMP
            "#,
        metadata.author,
        metadata.title,
        metadata.clean_title,
        metadata.file_path,
        metadata.file_name,
        metadata.path_parent,
        metadata.series,
        metadata.clean_series,
        metadata.series_part,
        metadata.cover_art,
        metadata.pub_year,
        metadata.narrated_by,
        metadata.duration,
        metadata.track_number,
        metadata.disc_number,
        metadata.file_size,
        metadata.mime_type,
        metadata.channels,
        metadata.sample_rate,
        metadata.bitrate,
        metadata.dramatized,
        metadata.extracts,
        rawmet, //metadata.raw_metadata,
        resolve_status,
        metadata.hash
    )
    .execute(db)
    .await;

    if let Err(e) = save_res {
        tracing::error!("Failed to save {}", e);
        return Err(ApiError::Database(e));
    }

    Ok(())
}

pub async fn group_title_cleanup_multipart(db: &Pool<Sqlite>) -> Result<(), ApiError> {
    let rows = sqlx::query!(
        r#"
        SELECT series as og_series, clean_series as series, author, title, id, file_path, path_parent
        FROM file_scan_cache
        ORDER BY series, author, title
        "#
    )
    .fetch_all(db)
    .await?;

    for row in rows {}

    Ok(())
}

pub async fn group_meta_fetch(
    db: &Pool<Sqlite>,
) -> Result<HashMap<String, HashMap<String, Vec<FileInfo>>>, ApiError> {
    let rows = sqlx::query!(
        r#"
            WITH
                files AS (
                    SELECT
                        id,
                        author,
                        title,
                        clean_series,
                        series,
                        file_name,
                        path_parent,
                        file_path
                    FROM file_scan_cache
                ),
                grouped AS (
                    SELECT author, clean_series, COUNT(*) AS cnt
                    FROM files
                    GROUP BY
                        author,
                        clean_series
                )
            SELECT f.*
            FROM files f
                JOIN grouped g ON f.author = g.author
                AND f.clean_series = g.clean_series
            WHERE
                g.cnt > 1
            ORDER BY f.author;
        "#
    )
    .fetch_all(db)
    .await?;

    let mut result: HashMap<String, HashMap<String, Vec<FileInfo>>> = HashMap::new();

    for row in rows {
        let series = row.series.unwrap_or_else(|| "unknown".to_string());
        let author = row.author.unwrap_or_else(|| "unknown".to_string());
        let id = row.id.unwrap_or_else(|| -1); // or some default ID
        let file_name = row.file_name;
        let title = row.title.unwrap_or_else(|| "unknown".to_string());
        let file_path = row.file_path;
        let path_parent = row.path_parent;
        let clean_series = row.clean_series.unwrap_or_else(|| "unknown".to_string());

        let author_entry = result
            .entry(author.trim().to_lowercase())
            .or_insert_with(|| HashMap::new());

        let file_info = FileInfo {
            id: id,
            file_name: file_name,
            series: series,
            title: title,
            path_parent: path_parent,
            file_path: file_path,
        };

        let book_entry = author_entry
            .entry(clean_series)
            .or_insert_with(|| Vec::new());
        book_entry.push(file_info);
    }

    let res_json = serde_json::to_string_pretty(&result).unwrap_or_default();
    let json_bytes = res_json.as_bytes();
    let mut json_file = fs::File::create("bookresmultipart.json").await?;
    json_file.write_all(json_bytes).await?;

    Ok(result)
}
