use sqlx::{Pool, Sqlite};

use crate::models::user::{Progress, ProgressUpdate};

pub async fn list_inprogress_db(db: &Pool<Sqlite>, user_id: i64) -> sqlx::Result<Vec<Progress>> {
    sqlx::query_as::<_, Progress>(
        r#"
    SELECT id, user_id, book_id, file_id, progress_ms, complete, updated_at
    FROM progress
    WHERE user_id = ?1
    ORDER BY updated_at DESC
    "#,
    )
    .bind(user_id)
    .fetch_all(db)
    .await
}

pub async fn get_progress_by_fileid(
    db: &Pool<Sqlite>,
    user_id: i64,
    book_id: i64,
    file_id: i64,
) -> sqlx::Result<Option<Progress>> {
    sqlx::query_as::<_, Progress>(
        r#"
    SELECT id, user_id, book_id, file_id, progress_ms, complete, updated_at
    FROM progress
    WHERE user_id = ?1 AND book_id = ?2 AND file_id = ?3
    "#,
    )
    .bind(user_id)
    .bind(book_id)
    .bind(file_id)
    .fetch_optional(db)
    .await
}

pub async fn get_progress_by_bookid(
    db: &Pool<Sqlite>,
    user_id: i64,
    book_id: i64,
) -> sqlx::Result<Vec<Progress>> {
    sqlx::query_as::<_, Progress>(
        r#"
    SELECT id, user_id, book_id, file_id, progress_ms, complete, updated_at
    FROM progress
    WHERE user_id = ?1 AND book_id = ?2
    "#,
    )
    .bind(user_id)
    .bind(book_id)
    .fetch_all(db)
    .await
}

pub async fn upsert_progress(
    db: &Pool<Sqlite>,
    user_id: i64,
    p: &ProgressUpdate,
) -> sqlx::Result<()> {
    // Formatted to match CURRENT_TIMESTAMP ("YYYY-MM-DD HH:MM:SS", UTC) so the
    // lexicographic WHERE comparison below stays valid across both sources; the
    // extra .3f millisecond suffix still compares correctly against second-
    // precision rows ("...:00" < "...:00.500") and keeps rapid same-second saves
    // ordered.
    let client_updated_at = p
        .updated_at
        .map(|t| t.format("%Y-%m-%d %H:%M:%S%.3f").to_string());
    sqlx::query!(
        r#"
        INSERT INTO progress (user_id, book_id, file_id, progress_ms, complete, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, COALESCE(?6, CURRENT_TIMESTAMP))
        ON CONFLICT(user_id, book_id, file_id) DO UPDATE SET
            progress_ms = excluded.progress_ms,
            complete = excluded.complete,
            updated_at = excluded.updated_at
        WHERE excluded.updated_at > progress.updated_at
        "#,
        user_id,
        p.book_id,
        p.file_id,
        p.progress_ms,
        p.complete,
        client_updated_at
    )
    .execute(db)
    .await?;
    Ok(())
}
