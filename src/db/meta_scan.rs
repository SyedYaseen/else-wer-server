use sqlx::{Pool, Sqlite};

use crate::{api::api_error::ApiError, models::fs_models::FileScanCache};

pub async fn save_meta(db: &Pool<Sqlite>, metadata: FileScanCache) -> Result<(), ApiError> {
    let resolve_status = metadata.resolve_status.value();
    let rawmet = "".to_owned();
    let save_res = sqlx::query!(
        r#"
            INSERT INTO file_scan_cache (
                author, title, clean_title, file_path, file_name, series, clean_series, series_part, 
                cover_art, pub_year, narrated_by, duration, track_number, 
                disc_number, file_size, mime_type, channels, sample_rate, 
                bitrate, dramatized, extracts,  raw_metadata, resolve_status, hash
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, 
                $15, $16, $17, $18, $19, $20, $21, $22, $23, $24
            )
            ON CONFLICT(file_path) DO UPDATE SET
                author = excluded.author,
                title = excluded.title,
                clean_title = excluded.clean_title,
                file_name = excluded.file_name,
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
