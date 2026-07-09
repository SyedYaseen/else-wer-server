use crate::{
    api::api_error::ApiError,
    db::series::upsert_series,
    file_ops::{
        book_meta_file::{BookMetaFile, read_book_meta_json, write_book_meta_json},
        grouping::{BookGroup, GroupRow, group_files, keys_similar},
        meta_cleanup::fold_key,
    },
    models::meta_scan::{ChangeDto, ChangeType, FileInfo, FileScanCache},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Pool, QueryBuilder, Sqlite, SqliteConnection, SqlitePool};
use std::collections::{HashMap, HashSet};
use tokio::{fs, io::AsyncWriteExt};

pub async fn cache_row_count(db: &Pool<Sqlite>) -> Result<i64, ApiError> {
    let row = sqlx::query!(
        r#"
        SELECT COUNT(id) as count
        FROM files
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

/// Group one scan-pass chunk of in-memory file records into books (folder = identity)
/// and attach their files to `files`. Returns the number of new `files` rows inserted.
///
/// scan_files feeds this in 500-row chunks, so a folder can be split across calls:
/// later chunks re-resolve the same book via files_location + album similarity and
/// just attach more files. Each chunk's grouping + attach runs in one transaction so a
/// mid-chunk failure can't leave a book without its files (a clean retry next scan).
pub async fn group_and_attach_files(
    db: &SqlitePool,
    chunk: &[FileScanCache],
) -> Result<u64, ApiError> {
    if chunk.is_empty() {
        return Ok(0);
    }

    let group_rows: Vec<GroupRow> = chunk
        .iter()
        .enumerate()
        .map(|(i, r)| GroupRow {
            row_idx: i,
            author: r.author.clone(),
            narrated_by: r.narrated_by.clone(),
            clean_series: r.clean_series.clone(),
            clean_title: r.clean_title.clone(),
            path_parent: r.path_parent.clone(),
            cover_art: r.cover_art.clone(),
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

    let mut tx = db.begin().await?;
    let mut attached: u64 = 0;
    // Series linking needs `db_pool` (upsert_series dedupes across the whole table),
    // so it runs after commit; collect (book_id, meta) pairs while grouping.
    let mut pending_series_links: Vec<(i64, BookMetaFile)> = Vec::new();

    for mut group in groups {
        let mut lock_book = false;

        let meta = if folder_group_count.get(&group.path_parent) == Some(&1) {
            read_book_meta_json(&group.path_parent).await
        } else {
            None
        };

        if let Some(meta) = &meta {
            group.title = meta.title.clone();
            group.author = meta.author.clone();
            if let Some(series) = meta.series.clone() {
                group.series = series;
            }
            group.narrated_by = meta.narrated_by.clone().or(group.narrated_by.take());
            lock_book = meta.user_locked;
        }
        let meta_applied = meta.is_some();

        let book_id = resolve_book_id(&mut tx, &group, lock_book, meta_applied).await?;
        attached += attach_files(&mut tx, book_id, chunk, &group.row_indices).await?;
        if let Some(meta) = meta {
            pending_series_links.push((book_id, meta));
        }
    }

    update_duration_file_sz(&mut *tx).await?;
    tx.commit().await?;

    for (book_id, meta) in &pending_series_links {
        restore_series_link(db, *book_id, meta).await?;
    }

    Ok(attached)
}

/// Re-link a book to its real-world series from the folder's metadata.json. Fill-only
/// (`series_id IS NULL` guard) so a mid-library rescan never regresses a newer DB-side
/// assignment; touches only series_id/series_sequence/series_locked — resolve_book_id
/// reads none of these, so scan bucketing is unaffected.
async fn restore_series_link(
    pool: &Pool<Sqlite>,
    book_id: i64,
    meta: &BookMetaFile,
) -> Result<(), ApiError> {
    let name = meta
        .series_name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty());

    match name {
        Some(name) => {
            let provider = if meta.series_asin.is_some() { "audible" } else { "user" };
            let sid = upsert_series(pool, name, provider, meta.series_asin.as_deref()).await?;
            sqlx::query(
                r#"
                UPDATE audiobooks SET series_id = ?, series_sequence = ?, series_locked = ?
                WHERE id = ? AND series_id IS NULL AND series_locked = 0
                "#,
            )
            .bind(sid)
            .bind(meta.series_sequence.as_deref())
            .bind(meta.series_locked)
            .bind(book_id)
            .execute(pool)
            .await?;
        }
        // User explicitly cleared the series; keep the auto-pass from re-adding one.
        None if meta.series_locked => {
            sqlx::query(
                "UPDATE audiobooks SET series_locked = 1 WHERE id = ? AND series_id IS NULL",
            )
            .bind(book_id)
            .execute(pool)
            .await?;
        }
        None => {}
    }

    Ok(())
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
    conn: &mut SqliteConnection,
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
    .fetch_all(&mut *conn)
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
            && (group.row_indices.len() > 1 || from_meta_file)
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
            .execute(&mut *conn)
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
    .fetch_one(&mut *conn)
    .await?;

    Ok(id)
}

/// Insert this group's files as VALUES rows (no more file_scan_cache to SELECT from).
/// UNIQUE(file_path) makes INSERT OR IGNORE the dedupe for concurrent/duplicate scans.
/// Returns the number of new rows actually inserted.
async fn attach_files(
    conn: &mut SqliteConnection,
    book_id: i64,
    chunk: &[FileScanCache],
    row_indices: &[usize],
) -> Result<u64, ApiError> {
    if row_indices.is_empty() {
        return Ok(0);
    }

    let mut qb = QueryBuilder::new(
        "INSERT OR IGNORE INTO files (book_id, file_name, file_path, duration, file_size, channels, sample_rate, bitrate, track_number, disc_number) ",
    );
    qb.push_values(row_indices.iter().map(|&i| &chunk[i]), |mut b, row| {
        b.push_bind(book_id)
            .push_bind(&row.file_name)
            .push_bind(&row.file_path)
            .push_bind(row.duration)
            .push_bind(row.file_size)
            .push_bind(row.channels)
            .push_bind(row.sample_rate)
            .push_bind(row.bitrate)
            .push_bind(row.track_number)
            .push_bind(row.disc_number);
    });

    let result = qb.build().execute(conn).await?;
    Ok(result.rows_affected())
}

async fn update_duration_file_sz<'e, E>(executor: E) -> Result<(), ApiError>
where
    E: sqlx::Executor<'e, Database = Sqlite>,
{
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
    .execute(executor)
    .await?;

    Ok(())
}

// Moved/removed files on disk: current known file_path -> files.id, so scan_files can
// diff disk paths against it (skip re-probing, detect deletions).
pub async fn fetch_known_file_paths(db: &Pool<Sqlite>) -> Result<HashMap<String, i64>, ApiError> {
    let rows = sqlx::query_as::<_, FileScanCacheFilePaths>(
        r#"
        SELECT id, file_path
        FROM files
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

/// Delete `files` rows no longer present on disk, then any audiobooks left with no
/// files. `progress` rows cascade via the files(id) ON DELETE CASCADE FK. One
/// transaction so the two deletes can't observe each other half-done.
pub async fn delete_removed_files(db: &Pool<Sqlite>, delete_ids: &[i64]) -> Result<u64, ApiError> {
    if delete_ids.is_empty() {
        return Ok(0);
    }

    let mut tx = db.begin().await?;

    let mut del_files_qb = QueryBuilder::new("DELETE FROM files");
    del_files_qb = bind_ids(del_files_qb, "id", delete_ids);
    let df = del_files_qb.build().execute(&mut *tx).await?;

    let db_del = sqlx::query(
        r#"
        DELETE FROM audiobooks
        WHERE id IN (
            SELECT b.id
            FROM audiobooks b
            LEFT JOIN files f ON b.id = f.book_id
            WHERE f.book_id IS NULL
        )
        "#,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    tracing::info!(
        deleted_files = df.rows_affected(),
        deleted_books = db_del.rows_affected(),
        "removed files no longer present on disk"
    );
    Ok(df.rows_affected())
}

pub async fn get_grouped_files(
    db: &Pool<Sqlite>,
) -> Result<HashMap<String, HashMap<String, Vec<FileInfo>>>, ApiError> {
    let rows = sqlx::query_as::<_, FileInfo>(
        r#"
            SELECT f.id as id, b.id as book_id, b.author, f.file_name, b.series, b.title, b.files_location as path_parent, f.file_path
            FROM files f JOIN audiobooks b ON f.book_id = b.id;
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
    ids: &'a [i64],
) -> QueryBuilder<'a, sqlx::Sqlite> {
    qb.push(format!(" WHERE {id_name} IN ("));

    let mut separated = qb.separated(", ");
    for id in ids {
        separated.push_bind(id);
    }

    qb.push(")");
    qb
}

/// Save org-UI edits. `change.file_ids` are `files.id` values (the single file-id
/// space; no more fsc echo). See ChangeType docs for what each variant does.
pub async fn save_user_file_org_changes(
    pool: &SqlitePool,
    changes: Vec<ChangeDto>,
) -> Result<(), ApiError> {
    let abk_q = "UPDATE audiobooks SET ";
    let files_q = "UPDATE files SET ";

    // Books the user touched get their folder metadata.json refreshed at the end.
    let mut touched_books: HashSet<i64> = HashSet::new();

    for change in changes {
        let mut files_qb: QueryBuilder<'_, Sqlite> = QueryBuilder::new(files_q);
        let mut abk_qb: QueryBuilder<'_, Sqlite> = QueryBuilder::new(abk_q);

        match change.change_type {
            ChangeType::FileMove => {
                // SELECT FROM ABK WHERE AUTHOR = NEW_AUTHOR AND ID = NEW_BOOK_ID
                // IF NONE => INSERT, ELSE UPDATE

                if change.new_author.is_none()
                    || change.new_book_id.is_none()
                    || change.new_series.is_none()
                    || change.file_ids.is_empty()
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

                // negative book_ids from UI indicate new book create
                if dest_book_id < 0 {
                    // files_location for the new book = parent dir of one of the
                    // moved files' current path (cover art is left to cover_links).
                    let file_path: Option<String> = match change.file_ids.first() {
                        Some(id) => sqlx::query_scalar("SELECT file_path FROM files WHERE id = ?")
                            .bind(id)
                            .fetch_optional(pool)
                            .await?,
                        None => None,
                    };
                    let files_location = file_path
                        .as_deref()
                        .and_then(|p| std::path::Path::new(p).parent())
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();

                    let insert = sqlx::query(
                        r#"
                        INSERT OR IGNORE INTO audiobooks (author, series, title, files_location, user_locked, created_at, updated_at)
                        VALUES (?, ?, ?, ?, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                        "#,
                    )
                    .bind(&dest_author)
                    .bind(&dest_series)
                    .bind(&dest_series)
                    .bind(&files_location)
                    .execute(pool)
                    .await?;
                    dest_book_id = insert.last_insert_rowid();
                }

                files_qb.push("book_id=").push_bind(dest_book_id);
                files_qb = bind_ids(files_qb, "id", &change.file_ids);
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
                    files_qb = bind_ids(files_qb, "id", &change.file_ids);
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
                    abk_parts.push(("author", new_author));
                }

                if let Some(new_file_title) = change.new_filetitle {
                    files_qb.push("file_name =").push_bind(new_file_title);
                    files_has_update = true;
                }

                if let Some(new_series) = change.new_series {
                    abk_parts.push(("series", new_series.clone()));
                    abk_parts.push(("title", new_series));
                }

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
                    abk_qb = bind_ids(abk_qb, "id", &change.file_ids);

                    abk_qb.push(")").build().execute(pool).await?;

                    let mut ids_qb =
                        QueryBuilder::new("SELECT DISTINCT book_id FROM files");
                    ids_qb = bind_ids(ids_qb, "id", &change.file_ids);
                    let renamed: Vec<i64> = ids_qb
                        .build_query_scalar()
                        .fetch_all(pool)
                        .await?;
                    touched_books.extend(renamed);
                }

                // Files
                if files_has_update {
                    files_qb
                        .push(" WHERE id = ")
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
        group_and_attach_files(pool, rows).await.unwrap();
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

        // fetch_known_file_paths would have removed these paths from scan_files'
        // rescan chunk (skip-set), but a rescan of the same rows must still be a
        // no-op if it ever happens (e.g. a race) — UNIQUE(file_path) dedupes.
        run_sync(&pool, &rows).await;

        assert_eq!(book_count(&pool).await, 1);
        assert_eq!(file_count(&pool, "the children of húrin").await, 10);
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

        // Rescan finds the same files again with their original (pre-lock) tags.
        run_sync(&pool, &rows).await;

        assert_eq!(book_count(&pool).await, 1);
        assert_eq!(file_count(&pool, "my curated title").await, 5);
    }

    #[tokio::test]
    async fn org_rename_locks_book_and_survives_rescan() {
        let pool = test_pool().await;
        // A real dir (not the usual fake "/lib/..." path): a rescan can no longer fall
        // back on file_scan_cache having been mutated in place by the org save (that
        // table's gone), so the folder's metadata.json — written below by
        // save_user_file_org_changes — is what makes the rename stick across rescans.
        let dir = temp_book_dir("org_rename_lock");
        let rows: Vec<FileScanCache> = (0..3)
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

        let file_ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM files")
            .fetch_all(&pool)
            .await
            .unwrap();

        let change = ChangeDto {
            change_type: ChangeType::Rename,
            file_ids,
            current_book_ids: None,
            new_book_id: None,
            current_author: None,
            current_series: None,
            current_filetitle: None,
            new_author: None,
            new_series: Some("narn i chîn húrin".to_string()),
            new_filetitle: None,
        };
        save_user_file_org_changes(&pool, vec![change]).await.unwrap();

        let locked: bool = sqlx::query_scalar("SELECT user_locked FROM audiobooks")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(locked);

        // Rescan with the original (pre-rename) tags must not undo the user's edit.
        run_sync(&pool, &rows).await;

        assert_eq!(book_count(&pool).await, 1);
        assert_eq!(file_count(&pool, "narn i chîn húrin").await, 3);

        std::fs::remove_dir_all(&dir).ok();
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
    async fn scan_restores_series_link_from_metadata_json() {
        let pool = test_pool().await;
        let dir = temp_book_dir("meta_series");
        std::fs::write(
            std::path::Path::new(&dir).join("metadata.json"),
            r#"{"version":1,"title":"the two towers","author":"j.r.r. tolkien",
                "series_name":"The Lord of the Rings","series_sequence":"2",
                "series_asin":"B005NF6MIQ","series_locked":true}"#,
        )
        .unwrap();

        let rows: Vec<FileScanCache> = (0..2)
            .map(|i| {
                fsc(
                    &dir,
                    &format!("{i:03}.mp3"),
                    "j.r.r. tolkien",
                    Some("the two towers"),
                    Some(&format!("chapter {i}")),
                )
            })
            .collect();
        run_sync(&pool, &rows).await;

        let (series_id, sequence, series_locked): (Option<i64>, Option<String>, bool) =
            sqlx::query_as("SELECT series_id, series_sequence, series_locked FROM audiobooks")
                .fetch_one(&pool)
                .await
                .unwrap();
        let sid = series_id.expect("book linked to series");
        assert_eq!(sequence.as_deref(), Some("2"));
        assert!(series_locked);

        let (name, provider, provider_id): (String, Option<String>, Option<String>) =
            sqlx::query_as("SELECT name, provider, provider_id FROM series WHERE id = ?")
                .bind(sid)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(name, "The Lord of the Rings");
        assert_eq!(provider.as_deref(), Some("audible"));
        assert_eq!(provider_id.as_deref(), Some("B005NF6MIQ"));

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

        let file_ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM files")
            .fetch_all(&pool)
            .await
            .unwrap();
        let change = ChangeDto {
            change_type: ChangeType::Rename,
            file_ids,
            current_book_ids: None,
            new_book_id: None,
            current_author: None,
            current_series: None,
            current_filetitle: None,
            new_author: None,
            new_series: Some("narn i chîn húrin".to_string()),
            new_filetitle: None,
        };
        save_user_file_org_changes(&pool, vec![change]).await.unwrap();

        let meta = read_book_meta_json(&dir).await.expect("metadata.json written");
        assert_eq!(meta.title, "narn i chîn húrin");
        assert!(meta.user_locked);

        std::fs::remove_dir_all(&dir).ok();
    }
}
