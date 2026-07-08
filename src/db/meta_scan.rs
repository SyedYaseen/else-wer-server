use crate::{
    api::api_error::ApiError,
    file_ops::{
        book_meta_file::{read_book_meta_json, write_book_meta_json},
        grouping::{BookGroup, GroupRow, group_files, keys_similar},
        meta_cleanup::fold_key,
    },
    models::meta_scan::{ChangeDto, ChangeType, FileInfo, FileScanCache, ResolvedStatus},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Pool, QueryBuilder, Sqlite, SqlitePool};
use std::collections::{HashMap, HashSet};
use tokio::{fs, io::AsyncWriteExt};

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

// Init scan / UI upload / Move or Add files on disk
pub async fn sync_disk_db_state(
    db: &Pool<Sqlite>,
    metadata_list: &[FileScanCache],
) -> Result<u64, ApiError> {
    let count = save_metadata_to_cache(db, metadata_list).await?;
    let processed_ids = group_and_attach_files(db).await?;
    update_fsc_resolved_status(db, &processed_ids).await?;
    update_duration_file_sz(db).await?;
    Ok(count)
}

pub async fn save_metadata_to_cache(
    db: &Pool<Sqlite>,
    metadata_list: &[FileScanCache],
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
    for _ in metadata_list {
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
    for m in metadata_list.iter() {
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

    let result = q.execute(db).await?;
    Ok(result.rows_affected())
}

#[derive(FromRow)]
struct GroupRowDb {
    id: i64,
    author: Option<String>,
    narrated_by: Option<String>,
    clean_series: Option<String>,
    clean_title: Option<String>,
    path_parent: String,
    cover_art: Option<String>,
}

/// Group unresolved cache rows into books (folder = identity) and attach their files.
/// Returns the fsc ids that were processed.
///
/// scan_files feeds sync_disk_db_state in 500-row chunks, so a folder can be split
/// across calls: later chunks re-resolve the same book via files_location + album
/// similarity and just attach more files.
pub async fn group_and_attach_files(pool: &SqlitePool) -> Result<Vec<i64>, ApiError> {
    let rows = sqlx::query_as::<_, GroupRowDb>(
        r#"
        SELECT id, author, narrated_by, clean_series, clean_title, path_parent, cover_art
        FROM file_scan_cache
        WHERE resolve_status = 0
        "#,
    )
    .fetch_all(pool)
    .await?;

    let group_rows: Vec<GroupRow> = rows
        .into_iter()
        .map(|r| GroupRow {
            fsc_id: r.id,
            author: r.author,
            narrated_by: r.narrated_by,
            clean_series: r.clean_series,
            clean_title: r.clean_title,
            path_parent: r.path_parent,
            cover_art: r.cover_art,
        })
        .collect();

    let groups = group_files(group_rows);

    // A folder's metadata.json (user edits / applied matches, portable across DB
    // wipes) outranks tag-derived values — but only when the folder unambiguously
    // maps to one book in this batch.
    let mut folder_group_count: HashMap<String, usize> = HashMap::new();
    for g in &groups {
        *folder_group_count.entry(g.path_parent.clone()).or_default() += 1;
    }

    let mut processed_ids: Vec<i64> = Vec::new();

    for mut group in groups {
        let mut lock_book = false;
        let mut meta_applied = false;

        if folder_group_count.get(&group.path_parent) == Some(&1)
            && let Some(meta) = read_book_meta_json(&group.path_parent).await
        {
            group.title = meta.title;
            group.author = meta.author;
            if let Some(series) = meta.series {
                group.series = series;
            }
            group.narrated_by = meta.narrated_by.or(group.narrated_by);
            lock_book = meta.user_locked;
            meta_applied = true;
        }

        let book_id = resolve_book_id(pool, &group, lock_book, meta_applied).await?;
        attach_files(pool, book_id, &group.fsc_ids).await?;
        processed_ids.extend(&group.fsc_ids);
    }

    Ok(processed_ids)
}

#[derive(FromRow)]
struct BookCandidate {
    id: i64,
    author: String,
    title: String,
    series: Option<String>,
    user_locked: bool,
}

/// Find or create the audiobooks row for a group. Multiple books may legitimately
/// share a files_location (loose single-file books), so candidates are picked by
/// album/title similarity, not first-row. `lock_book`/`from_meta_file` are set when
/// the group's values came from the folder's metadata.json.
async fn resolve_book_id(
    pool: &SqlitePool,
    group: &BookGroup,
    lock_book: bool,
    from_meta_file: bool,
) -> Result<i64, ApiError> {
    let candidates = sqlx::query_as::<_, BookCandidate>(
        r#"
        SELECT id, author, title, series, user_locked
        FROM audiobooks
        WHERE files_location = ?
        "#,
    )
    .bind(&group.path_parent)
    .fetch_all(pool)
    .await?;

    let series_key = fold_key(&group.series);
    let title_key = fold_key(&group.title);

    let found = candidates.iter().find(|c| {
        keys_similar(&fold_key(c.series.as_deref().unwrap_or_default()), &series_key)
            || keys_similar(&fold_key(&c.title), &title_key)
    });

    if let Some(book) = found {
        // Self-heal names computed from a partial folder (e.g. the first uploaded
        // chunk created the book) or restore them from metadata.json — but never
        // touch user-locked rows.
        if !book.user_locked
            && (group.fsc_ids.len() > 1 || from_meta_file)
            && (book.title != group.title || book.author != group.author)
        {
            let update = sqlx::query(
                r#"
                UPDATE audiobooks
                SET title = ?, author = ?, series = ?, narrated_by = COALESCE(?, narrated_by),
                    user_locked = (user_locked | ?)
                WHERE id = ?
                "#,
            )
            .bind(&group.title)
            .bind(&group.author)
            .bind(&group.series)
            .bind(&group.narrated_by)
            .bind(lock_book)
            .bind(book.id)
            .execute(pool)
            .await;

            if let Err(e) = update {
                let unique = e
                    .as_database_error()
                    .map(|d| d.is_unique_violation())
                    .unwrap_or(false);
                if unique {
                    tracing::warn!(
                        "Keeping existing name for book {}: rename to '{} / {}' collides",
                        book.id,
                        group.author,
                        group.title
                    );
                } else {
                    return Err(e.into());
                }
            }
        }
        return Ok(book.id);
    }

    // The same book at another files_location stays a separate row on purpose — the
    // user merges editions (m4b vs MP3 folder) manually via the org UI. The conflict
    // target only guards re-inserting this exact folder+title (e.g. rescan races).
    let id = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO audiobooks (author, series, title, files_location, cover_art, narrated_by, user_locked, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        ON CONFLICT(files_location, title) DO UPDATE SET updated_at = CURRENT_TIMESTAMP
        RETURNING id
        "#,
    )
    .bind(&group.author)
    .bind(&group.series)
    .bind(&group.title)
    .bind(&group.path_parent)
    .bind(&group.cover_art)
    .bind(&group.narrated_by)
    .bind(lock_book)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

async fn attach_files(pool: &SqlitePool, book_id: i64, fsc_ids: &Vec<i64>) -> Result<(), ApiError> {
    if fsc_ids.is_empty() {
        return Ok(());
    }

    let mut qb = QueryBuilder::new(
        r#"
        INSERT OR IGNORE INTO files (book_id, file_id, file_name, file_path, duration, file_size, channels, sample_rate, bitrate)
        SELECT "#,
    );
    qb.push_bind(book_id);
    qb.push(
        r#", fsc.id, fsc.file_name, fsc.file_path, fsc.duration, fsc.file_size, fsc.channels, fsc.sample_rate, fsc.bitrate
        FROM file_scan_cache fsc"#,
    );
    qb = bind_ids(qb, "fsc.id", fsc_ids);
    qb.build().execute(pool).await?;

    Ok(())
}

async fn update_duration_file_sz(pool: &SqlitePool) -> Result<(), ApiError> {
    sqlx::query(
        r#"
WITH totals AS (
    SELECT 
        book_id, 
        SUM(file_size) AS sz, 
        SUM(duration) AS dur 
    FROM files 
    GROUP BY book_id
)
UPDATE audiobooks 
SET 
    book_size = totals.sz,
    duration = totals.dur
FROM totals
WHERE audiobooks.id = totals.book_id;
    "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_fsc_resolved_status(
    db: &SqlitePool,
    processed_ids: &Vec<i64>,
) -> Result<(), ApiError> {
    if processed_ids.is_empty() {
        return Ok(());
    }

    let mut qb = QueryBuilder::new(
        "UPDATE file_scan_cache SET resolve_status = ",
    );
    qb.push_bind(ResolvedStatus::AutoResolved.value());
    qb.push(", updated_at = CURRENT_TIMESTAMP");
    qb = bind_ids(qb, "id", processed_ids);
    qb.build().execute(db).await?;

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

pub async fn get_grouped_files(
    db: &Pool<Sqlite>,
) -> Result<HashMap<String, HashMap<String, Vec<FileInfo>>>, ApiError> {
    let rows = sqlx::query_as::<_, FileInfo>(
        r#"
            SELECT f.file_id as id, b.id as book_id, b.author, f.file_name, b.series, b.title, fsc.path_parent, f.file_path
            FROM files f JOIN audiobooks b ON f.book_id = b.id
            JOIN file_scan_cache fsc ON f.file_id = fsc.id;
        "#
    )
    .fetch_all(db)
    .await?;

    let mut result: HashMap<String, HashMap<String, Vec<FileInfo>>> = HashMap::new();

    for row in rows {
        let mut author = row.author.clone();

        if author.is_empty() {
            author = "unknown".to_string();
        }

        let mut series = row.series.clone();
        if series.is_empty() {
            series = "unknown".to_string();
        }

        let author_entry = result.entry(author).or_insert_with(|| HashMap::new());
        let book_entry = author_entry.entry(series).or_insert_with(|| Vec::new());
        book_entry.push(row);
    }

    // Debug artifact only - must not fail an otherwise-successful read.
    let res_json = serde_json::to_string_pretty(&result).unwrap_or_default();
    match fs::File::create("bookresmultipart.json").await {
        Ok(mut json_file) => {
            if let Err(e) = json_file.write_all(res_json.as_bytes()).await {
                tracing::warn!("Failed to write debug artifact bookresmultipart.json: {e}");
            }
        }
        Err(e) => {
            tracing::warn!("Failed to create debug artifact bookresmultipart.json: {e}");
        }
    }

    Ok(result)
}

fn bind_ids<'a>(
    mut qb: QueryBuilder<'a, sqlx::Sqlite>,
    id_name: &str,
    ids: &'a Vec<i64>,
) -> QueryBuilder<'a, sqlx::Sqlite> {
    qb.push(format!(" WHERE {id_name} IN ("));

    let mut separated = qb.separated(", ");
    for id in ids {
        separated.push_bind(id);
    }

    qb.push(")");
    qb
}

pub async fn save_user_file_org_changes_filescan_cache(
    pool: &SqlitePool,
    changes: Vec<ChangeDto>,
) -> Result<(), ApiError> {
    let fsc_q: &'static str = "UPDATE file_scan_cache SET resolve_status = 2,";
    let abk_q = "UPDATE audiobooks SET ";
    let files_q = "UPDATE files SET ";

    // Books the user touched get their folder metadata.json refreshed at the end.
    let mut touched_books: HashSet<i64> = HashSet::new();

    for change in changes {
        let mut fsc_qb = QueryBuilder::new(fsc_q);
        let mut files_qb: QueryBuilder<'_, Sqlite> = QueryBuilder::new(files_q);
        let mut abk_qb: QueryBuilder<'_, Sqlite> = QueryBuilder::new(abk_q);

        let mut fsc_parts: Vec<(&'static str, String)> = Vec::new();

        match change.change_type {
            ChangeType::FileMove => {
                // SELECT FROM ABK WHERE AUTHOR = NEW_AUTHOR AND ID = NEW_BOOK_ID
                // IF NONE => INSERT, ELSE UPDATE

                // UPDATE FSC -> Complete

                if change.new_author.is_none()
                    || change.new_book_id.is_none()
                    || change.new_series.is_none()
                    || change.file_ids.len() == 0
                {
                    tracing::warn!(
                        "Skipping invalid FileMove change (missing author/book_id/series/file_ids): {:?}",
                        change.file_ids
                    );
                    continue;
                }

                let dest_author = change.new_author.clone().unwrap();
                let dest_series = change.new_series.clone().unwrap();
                let mut dest_book_id = change.new_book_id.unwrap();

                // Update fsc
                fsc_parts.push(("author", dest_author.clone()));
                fsc_parts.push(("clean_series", dest_series.clone()));

                for (i, (field, value)) in fsc_parts.into_iter().enumerate() {
                    if i > 0 {
                        fsc_qb.push(", ");
                    }
                    fsc_qb.push(field).push(" = ").push_bind(value);
                }

                fsc_qb = bind_ids(fsc_qb, "id", &change.file_ids);
                fsc_qb.build().execute(pool).await?;

                // negative book_ids from UI indicate new book create
                if dest_book_id < 0 {
                    let mut insert_qb = QueryBuilder::new(
                        r#"
                        INSERT OR IGNORE INTO audiobooks (author, series, title, files_location, cover_art, metadata, duration, user_locked, created_at, updated_at)
                        SELECT
                            fsc.author,
                            fsc.clean_series,
                            fsc.clean_series,
                            fsc.path_parent,
                            fsc.cover_art,
                            fsc.raw_metadata,
                            fsc.duration,
                            1,
                            CURRENT_TIMESTAMP,
                            CURRENT_TIMESTAMP
                        FROM file_scan_cache fsc
                        "#,
                    );

                    insert_qb = bind_ids(insert_qb, "id", &change.file_ids);
                    let id = insert_qb.build().execute(pool).await?;
                    dest_book_id = id.last_insert_rowid();
                }

                files_qb.push("book_id=").push_bind(dest_book_id);
                files_qb = bind_ids(files_qb, "file_id", &change.file_ids);
                files_qb.build().execute(pool).await?;

                // User placed these files here deliberately; rescans must not undo it.
                sqlx::query("UPDATE audiobooks SET user_locked = 1 WHERE id = ?")
                    .bind(dest_book_id)
                    .execute(pool)
                    .await?;
                touched_books.insert(dest_book_id);

                QueryBuilder::new("DELETE FROM audiobooks WHERE id IN (SELECT b.id FROM audiobooks b LEFT JOIN files f ON b.id = f.book_id WHERE f.book_id IS NULL)")
                    .build()
                    .execute(pool)
                    .await?;
            }
            ChangeType::MergeTitle => {
                if let Some(dest_book_id) = change.new_book_id {
                    files_qb.push("book_id=").push_bind(dest_book_id);
                    files_qb = bind_ids(files_qb, "file_id", &change.file_ids);
                    files_qb.build().execute(pool).await?;

                    let mut prog_qb: QueryBuilder<'_, Sqlite> =
                        QueryBuilder::new("UPDATE PROGRESS SET book_id = ");
                    prog_qb.push_bind(dest_book_id);
                    prog_qb = bind_ids(prog_qb, "file_id", &change.file_ids);
                    prog_qb.build().execute(pool).await?;

                    // A user-made merge must survive rescans.
                    sqlx::query("UPDATE audiobooks SET user_locked = 1 WHERE id = ?")
                        .bind(dest_book_id)
                        .execute(pool)
                        .await?;
                    touched_books.insert(dest_book_id);
                }

                if let Some(curr_book_ids) = change.current_book_ids {
                    let mut abk_del_qb = QueryBuilder::new("DELETE FROM AUDIOBOOKS ");
                    abk_del_qb = bind_ids(abk_del_qb, "id", &curr_book_ids);
                    abk_del_qb.build().execute(pool).await?;
                }
            }
            ChangeType::MoveTitle | ChangeType::Rename => {
                let mut files_has_update = false;
                let mut abk_parts: Vec<(&'static str, String)> = Vec::new();

                if let Some(new_author) = change.new_author {
                    fsc_parts.push(("author", new_author.clone()));
                    abk_parts.push(("author", new_author));
                }

                if let Some(new_file_title) = change.new_filetitle {
                    fsc_parts.push(("file_name", new_file_title.clone()));
                    files_qb.push("file_name =").push_bind(new_file_title);
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
                fsc_qb.build().execute(pool).await?;

                if !abk_parts.is_empty() {
                    for (i, (field, value)) in abk_parts.into_iter().enumerate() {
                        if i > 0 {
                            abk_qb.push(", ");
                        }
                        abk_qb.push(field).push(" = ").push_bind(value);
                    }
                    // User renames must survive rescans.
                    abk_qb.push(", user_locked = 1");
                    abk_qb.push(" WHERE id in (SELECT book_id from files");
                    abk_qb = bind_ids(abk_qb, "file_id", &change.file_ids);

                    abk_qb.push(")").build().execute(pool).await?;

                    let mut ids_qb =
                        QueryBuilder::new("SELECT DISTINCT book_id FROM files");
                    ids_qb = bind_ids(ids_qb, "file_id", &change.file_ids);
                    let renamed: Vec<i64> = ids_qb
                        .build_query_scalar()
                        .fetch_all(pool)
                        .await?;
                    touched_books.extend(renamed);
                }

                // Files
                if files_has_update {
                    files_qb
                        .push(" WHERE file_id = ")
                        .push_bind(&change.file_ids.first())
                        .build()
                        .execute(pool)
                        .await?;
                }
            }
        }
    }

    update_duration_file_sz(pool).await?;

    for book_id in touched_books {
        write_book_meta_json(pool, book_id).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        // Single connection: each in-memory sqlite connection is its own database.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        pool
    }

    fn fsc(
        parent: &str,
        file_name: &str,
        author: &str,
        album: Option<&str>,
        title: Option<&str>,
    ) -> FileScanCache {
        let mut m = FileScanCache::new(
            format!("{parent}/{file_name}"),
            file_name.to_string(),
            parent.to_string(),
        );
        m.author = Some(author.to_string());
        m.series = album.map(|s| s.to_string());
        m.clean_series = album
            .map(|s| s.to_string())
            .or_else(|| title.map(|s| s.to_string()));
        m.clean_title = title.map(|s| s.to_string());
        m
    }

    async fn run_sync(pool: &SqlitePool, rows: &[FileScanCache]) {
        sync_disk_db_state(pool, rows).await.unwrap();
    }

    async fn book_count(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM audiobooks")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn file_count(pool: &SqlitePool, book_title: &str) -> i64 {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM files WHERE book_id = (SELECT id FROM audiobooks WHERE title = ?)",
        )
        .bind(book_title)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn hurin_folder_becomes_one_book() {
        let pool = test_pool().await;
        let rows: Vec<FileScanCache> = (0..128)
            .map(|i| {
                fsc(
                    "/lib/hurin/MP3",
                    &format!("{i:03}.mp3"),
                    "j.r.r. tolkien",
                    Some("the children of húrin"),
                    Some(&format!("chapter {i}")),
                )
            })
            .collect();
        run_sync(&pool, &rows).await;

        assert_eq!(book_count(&pool).await, 1);
        assert_eq!(file_count(&pool, "the children of húrin").await, 128);
    }

    #[tokio::test]
    async fn hobbit_junk_albums_fold_into_real_book() {
        let pool = test_pool().await;
        let mut rows: Vec<FileScanCache> = (0..18)
            .map(|i| {
                fsc(
                    "/lib/hobbit/MP31",
                    &format!("{i:02}.mp3"),
                    "rob inglis",
                    Some("rob inglis"),
                    Some(&format!("track {i}")),
                )
            })
            .collect();
        rows.push(fsc(
            "/lib/hobbit/MP31",
            "19.mp3",
            "j.r.r. tolkien",
            Some("the hobbit"),
            Some("track 19"),
        ));
        run_sync(&pool, &rows).await;

        assert_eq!(book_count(&pool).await, 1);
        assert_eq!(file_count(&pool, "the hobbit").await, 19);
    }

    #[tokio::test]
    async fn m4b_and_mp3_subfolder_stay_two_books() {
        let pool = test_pool().await;
        let mut rows: Vec<FileScanCache> = (0..3)
            .map(|i| {
                fsc(
                    "/lib/fellowship",
                    &format!("part{i}.m4b"),
                    "j.r.r. tolkien",
                    Some("the fellowship of the ring"),
                    Some(&format!("part {i}")),
                )
            })
            .collect();
        for i in 0..31 {
            rows.push(fsc(
                "/lib/fellowship/MP3",
                &format!("{i:02}.mp3"),
                "j.r.r. tolkien",
                Some(if i < 20 {
                    "the fellowship of the ring"
                } else {
                    "fellowship of the ring 01"
                }),
                Some(&format!("ch {i}")),
            ));
        }
        run_sync(&pool, &rows).await;

        assert_eq!(book_count(&pool).await, 2);
        let sizes: Vec<i64> = sqlx::query_scalar(
            "SELECT COUNT(*) FROM files GROUP BY book_id ORDER BY COUNT(*)",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(sizes, vec![3, 31]);
    }

    #[tokio::test]
    async fn loose_files_share_location_as_separate_books() {
        let pool = test_pool().await;
        let rows = vec![
            fsc("/lib/loose", "dune.mp3", "various", Some("dune"), Some("dune")),
            fsc(
                "/lib/loose",
                "neuro.mp3",
                "various",
                Some("neuromancer"),
                Some("neuromancer"),
            ),
            fsc(
                "/lib/loose",
                "hyperion.mp3",
                "various",
                Some("hyperion cantos"),
                Some("hyperion"),
            ),
            fsc(
                "/lib/loose",
                "martian.mp3",
                "various",
                Some("the martian by weir"),
                Some("the martian"),
            ),
        ];
        run_sync(&pool, &rows).await;

        assert_eq!(book_count(&pool).await, 4);
        let locs: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT files_location) FROM audiobooks",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(locs, 1);
    }

    #[tokio::test]
    async fn rescan_is_idempotent() {
        let pool = test_pool().await;
        let rows: Vec<FileScanCache> = (0..10)
            .map(|i| {
                fsc(
                    "/lib/hurin/MP3",
                    &format!("{i:03}.mp3"),
                    "j.r.r. tolkien",
                    Some("the children of húrin"),
                    Some(&format!("chapter {i}")),
                )
            })
            .collect();
        run_sync(&pool, &rows).await;

        // One row comes back unresolved (e.g. file touched on disk).
        sqlx::query("UPDATE file_scan_cache SET resolve_status = 0 WHERE file_name = '003.mp3'")
            .execute(&pool)
            .await
            .unwrap();
        run_sync(&pool, &[]).await;

        assert_eq!(book_count(&pool).await, 1);
        assert_eq!(file_count(&pool, "the children of húrin").await, 10);

        // Only that row was re-marked; nothing else got touched.
        let unresolved: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM file_scan_cache WHERE resolve_status != 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(unresolved, 0);
    }

    #[tokio::test]
    async fn chunked_upload_self_heals_book_name() {
        let pool = test_pool().await;
        // First chunk: a single chapter creates the book named after its track title.
        let first = vec![fsc(
            "/lib/hurin/MP3",
            "000.mp3",
            "j.r.r. tolkien",
            Some("the children of húrin"),
            Some("chapter 0"),
        )];
        run_sync(&pool, &first).await;
        assert_eq!(book_count(&pool).await, 1);

        // Remaining chapters arrive in a later chunk.
        let rest: Vec<FileScanCache> = (1..12)
            .map(|i| {
                fsc(
                    "/lib/hurin/MP3",
                    &format!("{i:03}.mp3"),
                    "j.r.r. tolkien",
                    Some("the children of húrin"),
                    Some(&format!("chapter {i}")),
                )
            })
            .collect();
        run_sync(&pool, &rest).await;

        assert_eq!(book_count(&pool).await, 1);
        assert_eq!(file_count(&pool, "the children of húrin").await, 12);
    }

    #[tokio::test]
    async fn user_locked_book_survives_rescan() {
        let pool = test_pool().await;
        let rows: Vec<FileScanCache> = (0..5)
            .map(|i| {
                fsc(
                    "/lib/hurin/MP3",
                    &format!("{i:03}.mp3"),
                    "j.r.r. tolkien",
                    Some("the children of húrin"),
                    Some(&format!("chapter {i}")),
                )
            })
            .collect();
        run_sync(&pool, &rows).await;

        sqlx::query("UPDATE audiobooks SET title = 'my curated title', user_locked = 1")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query("UPDATE file_scan_cache SET resolve_status = 0")
            .execute(&pool)
            .await
            .unwrap();
        run_sync(&pool, &[]).await;

        assert_eq!(book_count(&pool).await, 1);
        assert_eq!(file_count(&pool, "my curated title").await, 5);
    }

    #[tokio::test]
    async fn org_rename_locks_book_and_survives_rescan() {
        let pool = test_pool().await;
        let rows: Vec<FileScanCache> = (0..3)
            .map(|i| {
                fsc(
                    "/lib/hurin/MP3",
                    &format!("{i:03}.mp3"),
                    "j.r.r. tolkien",
                    Some("the children of húrin"),
                    Some(&format!("chapter {i}")),
                )
            })
            .collect();
        run_sync(&pool, &rows).await;

        let fsc_ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM file_scan_cache")
            .fetch_all(&pool)
            .await
            .unwrap();

        let change = ChangeDto {
            change_type: ChangeType::Rename,
            file_ids: fsc_ids,
            current_book_ids: None,
            new_book_id: None,
            current_author: None,
            current_series: None,
            current_filetitle: None,
            new_author: None,
            new_series: Some("narn i chîn húrin".to_string()),
            new_filetitle: None,
        };
        save_user_file_org_changes_filescan_cache(&pool, vec![change])
            .await
            .unwrap();

        let locked: bool = sqlx::query_scalar("SELECT user_locked FROM audiobooks")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(locked);

        // Rescan with the original (pre-rename) tags must not undo the user's edit.
        sqlx::query("UPDATE file_scan_cache SET resolve_status = 0")
            .execute(&pool)
            .await
            .unwrap();
        run_sync(&pool, &[]).await;

        assert_eq!(book_count(&pool).await, 1);
        assert_eq!(file_count(&pool, "narn i chîn húrin").await, 3);
    }

    fn temp_book_dir(name: &str) -> String {
        let dir = std::env::temp_dir().join(format!("elsewer_test_{}_{}", name, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.to_string_lossy().to_string()
    }

    #[tokio::test]
    async fn scan_honors_folder_metadata_json() {
        let pool = test_pool().await;
        let dir = temp_book_dir("meta_read");
        std::fs::write(
            std::path::Path::new(&dir).join("metadata.json"),
            r#"{"version":1,"title":"narn i chîn húrin","author":"j.r.r. tolkien","narrated_by":"christopher lee","user_locked":true}"#,
        )
        .unwrap();

        let rows: Vec<FileScanCache> = (0..4)
            .map(|i| {
                fsc(
                    &dir,
                    &format!("{i:03}.mp3"),
                    "wrong author",
                    Some("wrong album"),
                    Some(&format!("chapter {i}")),
                )
            })
            .collect();
        run_sync(&pool, &rows).await;

        let (title, author, narrated_by, locked): (String, String, Option<String>, bool) =
            sqlx::query_as(
                "SELECT title, author, narrated_by, user_locked FROM audiobooks",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(title, "narn i chîn húrin");
        assert_eq!(author, "j.r.r. tolkien");
        assert_eq!(narrated_by.as_deref(), Some("christopher lee"));
        assert!(locked);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn org_rename_writes_metadata_json() {
        let pool = test_pool().await;
        let dir = temp_book_dir("meta_write");

        let rows: Vec<FileScanCache> = (0..2)
            .map(|i| {
                fsc(
                    &dir,
                    &format!("{i:03}.mp3"),
                    "j.r.r. tolkien",
                    Some("the children of húrin"),
                    Some(&format!("chapter {i}")),
                )
            })
            .collect();
        run_sync(&pool, &rows).await;

        let fsc_ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM file_scan_cache")
            .fetch_all(&pool)
            .await
            .unwrap();
        let change = ChangeDto {
            change_type: ChangeType::Rename,
            file_ids: fsc_ids,
            current_book_ids: None,
            new_book_id: None,
            current_author: None,
            current_series: None,
            current_filetitle: None,
            new_author: None,
            new_series: Some("narn i chîn húrin".to_string()),
            new_filetitle: None,
        };
        save_user_file_org_changes_filescan_cache(&pool, vec![change])
            .await
            .unwrap();

        let meta = read_book_meta_json(&dir).await.expect("metadata.json written");
        assert_eq!(meta.title, "narn i chîn húrin");
        assert!(meta.user_locked);

        std::fs::remove_dir_all(&dir).ok();
    }
}
