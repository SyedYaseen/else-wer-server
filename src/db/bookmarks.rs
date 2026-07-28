use sqlx::{Pool, Sqlite};

use crate::models::bookmarks::{Bookmark, CreateBookmark};

pub async fn create_bookmark(
    db: &Pool<Sqlite>,
    user_id: i64,
    b: &CreateBookmark,
) -> sqlx::Result<Bookmark> {
    sqlx::query_as::<_, Bookmark>(
        r#"
        INSERT INTO bookmarks (user_id, book_id, file_id, timestamp_ms, note)
        VALUES (?1, ?2, ?3, ?4, ?5)
        RETURNING id, user_id, book_id, file_id, timestamp_ms, note, created_at
        "#,
    )
    .bind(user_id)
    .bind(b.book_id)
    .bind(b.file_id)
    .bind(b.timestamp_ms)
    .bind(&b.note)
    .fetch_one(db)
    .await
}

pub async fn list_bookmarks_by_book(
    db: &Pool<Sqlite>,
    user_id: i64,
    book_id: i64,
) -> sqlx::Result<Vec<Bookmark>> {
    sqlx::query_as::<_, Bookmark>(
        r#"
        SELECT id, user_id, book_id, file_id, timestamp_ms, note, created_at
        FROM bookmarks
        WHERE user_id = ?1 AND book_id = ?2
        ORDER BY file_id ASC, timestamp_ms ASC
        "#,
    )
    .bind(user_id)
    .bind(book_id)
    .fetch_all(db)
    .await
}

/// Scoped by user_id in the WHERE clause (not just checked post-fetch) so one
/// user can't delete another's bookmark by guessing an id. Returns rows
/// affected so the handler can 404 on 0.
pub async fn delete_bookmark(
    db: &Pool<Sqlite>,
    user_id: i64,
    bookmark_id: i64,
) -> sqlx::Result<u64> {
    let result = sqlx::query!(
        r#"
        DELETE FROM bookmarks WHERE id = ?1 AND user_id = ?2
        "#,
        bookmark_id,
        user_id
    )
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}
