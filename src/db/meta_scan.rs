use crate::{
    api::api_error::ApiError,
    models::meta_scan::{ChangeDto, ChangeType, FileInfo, FileScanCache, ResolvedStatus},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Pool, QueryBuilder, Sqlite, SqlitePool};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

pub async fn cache_row_count(db: &Pool<Sqlite>) -> Result<i64, ApiError> {
    let row = sqlx::query!(
        r#"
        SELECT COUNT(id) as count
        FROM file_scan_cache
        "#
    )
    .fetch_one(db)
    .await?;

    Ok(row.count)
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FileScanCacheFilePaths {
    pub id: i64,
    pub file_path: String,
}

// Init scan / UI upload / Moved files
pub async fn sync_disk_db_state(
    db: &Pool<Sqlite>,
    metadata_list: Vec<FileScanCache>,
) -> Result<u64, ApiError> {
    let count = save_metadata_to_cache(db, metadata_list).await?;
    insert_new_books_from_cache(db).await?;
    insert_new_files_from_cache(db).await?;
    update_fsc_resolved_status(db).await?;
    Ok(count)
}

pub async fn save_metadata_to_cache(
    db: &Pool<Sqlite>,
    metadata_list: Vec<FileScanCache>,
) -> Result<u64, ApiError> {
    if metadata_list.is_empty() {
        return Ok(0);
    }
    let rawmet = "".to_owned();
    let mut query = String::from(
        "INSERT OR IGNORE INTO file_scan_cache (
            author, title, clean_title, file_path, file_name, path_parent,
            series, clean_series, series_part, cover_art, pub_year, narrated_by,
            duration, track_number, disc_number, file_size, mime_type, channels,
            sample_rate, bitrate, dramatized, extracts, raw_metadata,
            resolve_status, hash
        ) VALUES ",
    );

    let mut first = true;
    for _ in &metadata_list {
        if !first {
            query.push_str(", ");
        }
        first = false;
        query.push_str(
            "(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        );
    }

    // Prepare query_with
    let mut q = sqlx::query_with(&query, sqlx::sqlite::SqliteArguments::default());

    // Bind all values
    for m in &metadata_list {
        let res_status = &m.resolve_status;
        q = q
            .bind(&m.author)
            .bind(&m.title)
            .bind(&m.clean_title)
            .bind(&m.file_path)
            .bind(&m.file_name)
            .bind(&m.path_parent)
            .bind(&m.series)
            .bind(&m.clean_series)
            .bind(&m.series_part)
            .bind(&m.cover_art)
            .bind(&m.pub_year)
            .bind(&m.narrated_by)
            .bind(&m.duration)
            .bind(&m.track_number)
            .bind(&m.disc_number)
            .bind(&m.file_size)
            .bind(&m.mime_type)
            .bind(&m.channels)
            .bind(&m.sample_rate)
            .bind(&m.bitrate)
            .bind(&m.dramatized)
            .bind(&m.extracts)
            .bind(&rawmet)
            .bind(res_status.value())
            .bind(&m.hash);
    }

    let result = q.execute(db).await.unwrap();
    Ok(result.rows_affected())
}

pub async fn insert_new_books_from_cache(pool: &SqlitePool) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO audiobooks (author, series, title, files_location, cover_art, metadata, duration, created_at, updated_at)
        SELECT
            fsc.author,
            fsc.clean_series,
            fsc.clean_series,
            fsc.path_parent,
            fsc.cover_art,
            fsc.raw_metadata,
            fsc.duration,
            CURRENT_TIMESTAMP,
            CURRENT_TIMESTAMP
        FROM file_scan_cache fsc
        WHERE fsc.resolve_status = 0 and fsc.author IS NOT NULL and fsc.clean_series is not null
        "#
    )
    .execute(pool)
    .await.unwrap();

    Ok(())
}

pub async fn insert_new_files_from_cache(pool: &SqlitePool) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO files (book_id, file_id, file_name, file_path, duration, channels, sample_rate, bitrate)
        SELECT
            ab.id AS book_id,
            fsc.id AS file_id,
            fsc.file_name,
            fsc.file_path,
            fsc.duration,
            fsc.channels,
            fsc.sample_rate,
            fsc.bitrate
        FROM
            file_scan_cache fsc
            JOIN audiobooks ab ON ab.author = fsc.author
            AND ab.title = fsc.clean_series
        WHERE
            fsc.resolve_status = 0
        "#
    )
    .execute(pool)
    .await.unwrap();

    Ok(())
}

pub async fn update_fsc_resolved_status(db: &SqlitePool) -> Result<(), ApiError> {
    let auto_resolved = ResolvedStatus::AutoResolved.value();
    sqlx::query!(
        r#"
        UPDATE file_scan_cache SET resolve_status = ?1
        "#,
        auto_resolved
    )
    .execute(db)
    .await?;

    Ok(())
}

// Moved files on disk
pub async fn fetch_all_stage_file_paths(
    db: &Pool<Sqlite>,
) -> Result<HashMap<String, i64>, ApiError> {
    let rows = sqlx::query_as::<_, FileScanCacheFilePaths>(
        r#"
        SELECT id, file_path 
        FROM file_scan_cache
        "#,
    )
    .fetch_all(db)
    .await?;

    let mut items: HashMap<String, i64> = HashMap::new();

    rows.into_iter().for_each(|r| {
        items.insert(r.file_path, r.id);
    });

    Ok(items)
}

pub async fn delete_removed_paths_from_cache(
    db: &Pool<Sqlite>,
    delete_fsc_ids: &[i64],
) -> Result<u64, ApiError> {
    if delete_fsc_ids.is_empty() {
        return Ok(0);
    }

    let placeholders = std::iter::repeat("?")
        .take(delete_fsc_ids.len())
        .collect::<Vec<_>>()
        .join(", ");

    let del_fsc_sql = format!("DELETE FROM file_scan_cache WHERE id IN ({})", placeholders);
    let del_files_sql = format!("DELETE FROM files WHERE file_id IN ({})", placeholders);
    let del_books_sql = "
    DELETE FROM audiobooks
    WHERE id IN (
        SELECT b.id
        FROM audiobooks b
        LEFT JOIN files f ON b.id = f.book_id
        WHERE f.book_id IS NULL
    )";

    let mut del_fsc_q = sqlx::query(&del_fsc_sql);
    let mut del_files_q = sqlx::query(&del_files_sql);

    for id in delete_fsc_ids {
        del_fsc_q = del_fsc_q.bind(id);
        del_files_q = del_files_q.bind(id);
    }

    let df = del_files_q.execute(db).await?;
    let result = del_fsc_q.execute(db).await?;
    let db_del = sqlx::query(del_books_sql).execute(db).await?;

    println!(
        "Del files: {} Del Fsc: {} Del Books: {}",
        df.rows_affected(),
        result.rows_affected(),
        db_del.rows_affected()
    );
    Ok(result.rows_affected())
}

pub async fn update_cache_metadata(
    db: &Pool<Sqlite>,
    metadata: FileScanCache,
) -> Result<(), ApiError> {
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

pub async fn get_grouped_files(
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

    // let res_json = serde_json::to_string_pretty(&result).unwrap_or_default();
    // let json_bytes = res_json.as_bytes();
    // let mut json_file = fs::File::create("bookresmultipart.json").await?;
    // json_file.write_all(json_bytes).await?;

    Ok(result)
}

fn bind_ids<'a>(
    mut qb: QueryBuilder<'a, sqlx::Sqlite>,
    id_name: &str,
    file_ids: &'a Vec<i64>,
) -> QueryBuilder<'a, sqlx::Sqlite> {
    qb.push(format!(" WHERE {id_name} IN ("));

    let mut separated = qb.separated(", ");
    for id in file_ids {
        separated.push_bind(id);
    }

    qb.push(")");
    qb
}

pub async fn save_user_file_org_changes_filescan_cache(
    pool: &SqlitePool,
    changes: Vec<ChangeDto>,
) -> Result<(), sqlx::Error> {
    let fsc_q: &'static str = "UPDATE file_scan_cache SET resolve_status = 2,";
    let abk_q = "UPDATE audiobooks SET ";
    let files_q = "UPDATE files SET file_name = ";

    for change in changes {
        let mut files_has_update = false;
        let mut fsc_qb = QueryBuilder::new(fsc_q);
        let mut files_qb: QueryBuilder<'_, Sqlite> = QueryBuilder::new(files_q);
        let mut abk_qb: QueryBuilder<'_, Sqlite> = QueryBuilder::new(abk_q);

        let mut fsc_parts: Vec<(&'static str, String)> = Vec::new();
        let mut abk_parts: Vec<(&'static str, String)> = Vec::new();

        if let Some(new_author) = change.new_author {
            fsc_parts.push(("author", new_author.clone()));
            abk_parts.push(("author", new_author));
        }

        if let Some(new_file_title) = change.new_filetitle {
            fsc_parts.push(("file_name", new_file_title.clone()));
            files_qb.push_bind(new_file_title);
            files_has_update = true;
        }

        if let Some(new_series) = change.new_series {
            fsc_parts.push(("clean_series", new_series.clone()));
            abk_parts.push(("series", new_series.clone()));
            abk_parts.push(("title", new_series));
        }

        for (i, (field, value)) in fsc_parts.into_iter().enumerate() {
            if i > 0 {
                fsc_qb.push(", ");
            }
            fsc_qb.push(field).push(" = ").push_bind(value);
        }

        // fsc
        fsc_qb = bind_ids(fsc_qb, "id", &change.file_ids);
        fsc_qb.build().execute(pool).await.unwrap();

        if !abk_parts.is_empty() {
            for (i, (field, value)) in abk_parts.into_iter().enumerate() {
                if i > 0 {
                    abk_qb.push(", ");
                }
                abk_qb.push(field).push(" = ").push_bind(value);
            }
            abk_qb.push(" WHERE id in (SELECT book_id from files");
            abk_qb = bind_ids(abk_qb, "file_id", &change.file_ids);
            abk_qb.push(")").build().execute(pool).await?;
        }

        // Files
        if files_has_update {
            files_qb
                .push("WHERE file_id = ")
                .push_bind(&change.file_ids.first())
                .build()
                .execute(pool)
                .await
                .unwrap();
        }
    }
    Ok(())
}
