use chrono::{NaiveDate, Utc};
use sqlx::{Pool, Sqlite};

use crate::models::stats::{DailyStat, FinishedBook};

// Periodic save cadence is 10s; this tolerates a delayed/backgrounded save without
// absorbing a real seek-sized jump into the aggregate.
const MAX_PLAUSIBLE_DELTA_MS: i64 = 5 * 60 * 1000;

pub fn sanitize_delta(delta_ms: i64) -> Option<i64> {
    if delta_ms <= 0 || delta_ms > MAX_PLAUSIBLE_DELTA_MS {
        return None;
    }
    Some(delta_ms)
}

pub async fn add_listened_ms(
    db: &Pool<Sqlite>,
    user_id: i64,
    book_id: i64,
    day: NaiveDate,
    delta_ms: i64,
) -> sqlx::Result<()> {
    let day_str = day.to_string();
    sqlx::query!(
        r#"
        INSERT INTO listening_stats_daily (user_id, book_id, day, ms_listened)
        VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(user_id, book_id, day) DO UPDATE SET
            ms_listened = ms_listened + excluded.ms_listened,
            updated_at = CURRENT_TIMESTAMP
        "#,
        user_id,
        book_id,
        day_str,
        delta_ms
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn daily_totals(
    db: &Pool<Sqlite>,
    user_id: i64,
    since_days: i64,
) -> sqlx::Result<Vec<DailyStat>> {
    let since = (Utc::now().date_naive() - chrono::Duration::days(since_days - 1)).to_string();
    let rows: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT day, SUM(ms_listened) as ms_listened
        FROM listening_stats_daily
        WHERE user_id = ?1 AND day >= ?2
        GROUP BY day
        ORDER BY day ASC
        "#,
    )
    .bind(user_id)
    .bind(since)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(day, ms_listened)| DailyStat { day, ms_listened })
        .collect())
}

pub async fn list_finished_books(
    db: &Pool<Sqlite>,
    user_id: i64,
) -> sqlx::Result<Vec<FinishedBook>> {
    sqlx::query_as::<_, FinishedBook>(
        r#"
        SELECT book_id, times_finished, finished_at
        FROM finished_books
        WHERE user_id = ?1
        ORDER BY finished_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(db)
    .await
}

/// True if book_id has files for this user and every one of them has complete=true.
/// Compares against files.book_id's row count (not just existing progress rows) so a
/// book whose first file was never played doesn't look finished.
pub async fn is_book_fully_complete(
    db: &Pool<Sqlite>,
    user_id: i64,
    book_id: i64,
) -> sqlx::Result<bool> {
    let (done,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM progress
        WHERE user_id = ?1 AND book_id = ?2 AND complete = 1
        "#,
    )
    .bind(user_id)
    .bind(book_id)
    .fetch_one(db)
    .await?;

    let (file_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM files WHERE book_id = ?1")
        .bind(book_id)
        .fetch_one(db)
        .await?;

    Ok(file_count > 0 && done == file_count)
}

/// Upserts finished_books for a genuine not-all-complete -> all-complete transition.
/// Caller must check is_book_fully_complete before and after the progress upsert and
/// only call this when it just flipped to true.
pub async fn record_finish_transition(
    db: &Pool<Sqlite>,
    user_id: i64,
    book_id: i64,
) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO finished_books (user_id, book_id, times_finished, finished_at)
        VALUES (?1, ?2, 1, CURRENT_TIMESTAMP)
        ON CONFLICT(user_id, book_id) DO UPDATE SET
            times_finished = times_finished + 1,
            finished_at = CURRENT_TIMESTAMP
        "#,
        user_id,
        book_id
    )
    .execute(db)
    .await?;
    Ok(())
}
