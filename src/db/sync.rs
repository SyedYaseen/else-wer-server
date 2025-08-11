use sqlx::{Pool, Sqlite};

use crate::models::user::{Progress, ProgressUpdate};

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

pub async fn upsert_progress(db: &Pool<Sqlite>, p: &ProgressUpdate) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO progress (user_id, book_id, file_id, progress_ms, complete)
        VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(user_id, book_id, file_id) DO UPDATE SET
            progress_ms = excluded.progress_ms,
            complete = excluded.complete,
            updated_at = CURRENT_TIMESTAMP
        WHERE user_id = excluded.user_id 
        AND book_id = excluded.book_id 
        AND file_id = excluded.file_id
        "#,
        p.user_id,
        p.book_id,
        p.file_id,
        p.progress_ms,
        p.complete
    )
    .execute(db)
    .await?;
    Ok(())
}
