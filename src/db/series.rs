use crate::api::api_error::ApiError;
use sqlx::SqlitePool;

pub async fn upsert_series(
    pool: &SqlitePool,
    name: &str,
    provider: &str,
    provider_id: Option<&str>,
) -> Result<i64, ApiError> {
    let id = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO series (name, provider, provider_id)
        VALUES (?, ?, ?)
        ON CONFLICT(name) DO UPDATE SET provider = excluded.provider, provider_id = excluded.provider_id
        RETURNING id
        "#,
    )
    .bind(name)
    .bind(provider)
    .bind(provider_id)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

/// Apply a user-chosen external match to a book. When title/author are applied the
/// row is user-locked so rescans keep the user's choice.
pub async fn set_book_match(
    pool: &SqlitePool,
    book_id: i64,
    series_id: Option<i64>,
    series_sequence: Option<&str>,
    asin: Option<&str>,
    title_author: Option<(&str, &str)>,
) -> Result<(), ApiError> {
    match title_author {
        Some((title, author)) => {
            sqlx::query(
                r#"
                UPDATE audiobooks
                SET series_id = ?, series_sequence = ?, asin = ?, title = ?, author = ?, user_locked = 1
                WHERE id = ?
                "#,
            )
            .bind(series_id)
            .bind(series_sequence)
            .bind(asin)
            .bind(title)
            .bind(author)
            .bind(book_id)
            .execute(pool)
            .await?;
        }
        None => {
            sqlx::query(
                r#"
                UPDATE audiobooks
                SET series_id = ?, series_sequence = ?, asin = ?
                WHERE id = ?
                "#,
            )
            .bind(series_id)
            .bind(series_sequence)
            .bind(asin)
            .bind(book_id)
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}

pub async fn get_book_title_author(
    pool: &SqlitePool,
    book_id: i64,
) -> Result<(String, String), ApiError> {
    let row = sqlx::query_as::<_, (String, String)>(
        r#"
        SELECT title, author
        FROM audiobooks
        WHERE id = ?
        "#,
    )
    .bind(book_id)
    .fetch_optional(pool)
    .await?;

    row.ok_or_else(|| ApiError::NotFound(format!("No book with id {book_id}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn match_apply_is_idempotent_and_locks_on_title_apply() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO audiobooks (author, series, title, files_location) VALUES ('t', 's', 'the silmarillion', '/lib/silmarillion')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let sid = upsert_series(&pool, "The Lord of the Rings", "audible", Some("B002V0QK4C"))
            .await
            .unwrap();
        let sid2 = upsert_series(&pool, "The Lord of the Rings", "audible", Some("B002V0QK4C"))
            .await
            .unwrap();
        assert_eq!(sid, sid2);

        set_book_match(&pool, 1, Some(sid), Some("0"), Some("B002V0QK4C"), None)
            .await
            .unwrap();
        let (series_id, locked): (Option<i64>, bool) =
            sqlx::query_as("SELECT series_id, user_locked FROM audiobooks WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(series_id, Some(sid));
        assert!(!locked);

        // Applying title/author locks the book.
        set_book_match(
            &pool,
            1,
            Some(sid),
            Some("0"),
            Some("B002V0QK4C"),
            Some(("The Silmarillion", "J. R. R. Tolkien")),
        )
        .await
        .unwrap();
        let (title, locked): (String, bool) =
            sqlx::query_as("SELECT title, user_locked FROM audiobooks WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(title, "The Silmarillion");
        assert!(locked);
    }
}
