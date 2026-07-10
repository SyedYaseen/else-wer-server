use crate::api::api_error::ApiError;
use sqlx::SqlitePool;

pub async fn upsert_series(
    pool: &SqlitePool,
    name: &str,
    provider: &str,
    provider_id: Option<&str>,
) -> Result<i64, ApiError> {
    // Provider id (e.g. the series ASIN) is the stable identity: books of one series can
    // come back with slightly different series title strings, so match on it first.
    if let Some(pid) = provider_id {
        let existing = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM series WHERE provider = ? AND provider_id = ?",
        )
        .bind(provider)
        .bind(pid)
        .fetch_optional(pool)
        .await?;

        if let Some(id) = existing {
            // Best-effort name refresh; on a UNIQUE(name) collision keep the old name.
            if let Err(e) = sqlx::query("UPDATE series SET name = ? WHERE id = ?")
                .bind(name)
                .bind(id)
                .execute(pool)
                .await
            {
                tracing::warn!("upsert_series: keeping old name for series {id}: {e}");
            }
            return Ok(id);
        }
    }

    // Name-keyed fallback; never null out an existing provider linkage.
    let id = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO series (name, provider, provider_id)
        VALUES (?, ?, ?)
        ON CONFLICT(name) DO UPDATE SET
            provider_id = COALESCE(excluded.provider_id, series.provider_id),
            provider = CASE WHEN excluded.provider_id IS NOT NULL THEN excluded.provider ELSE series.provider END
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

/// Manually link (or clear, when `series_id` is None) books' real-world series.
/// Sets series_locked so the automatic metadata pass never overrides the user's choice.
pub async fn assign_books_to_series(
    pool: &SqlitePool,
    series_id: Option<i64>,
    books: &[(i64, Option<String>)],
) -> Result<u64, ApiError> {
    let mut updated = 0;
    for (book_id, sequence) in books {
        let result = sqlx::query(
            r#"
            UPDATE audiobooks
            SET series_id = ?, series_sequence = ?, series_locked = 1
            WHERE id = ?
            "#,
        )
        .bind(series_id)
        .bind(sequence.as_deref())
        .bind(book_id)
        .execute(pool)
        .await?;
        updated += result.rows_affected();
    }
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn provider_id_dedups_across_name_variants() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        // Same series ASIN, different title strings -> one row, name refreshed.
        let a = upsert_series(&pool, "Lord of the Rings", "audible", Some("B005NF6MIQ"))
            .await
            .unwrap();
        let b = upsert_series(&pool, "The Lord of the Rings", "audible", Some("B005NF6MIQ"))
            .await
            .unwrap();
        assert_eq!(a, b);
        let name: String = sqlx::query_scalar("SELECT name FROM series WHERE id = ?")
            .bind(a)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(name, "The Lord of the Rings");

        // A name-keyed upsert without provider_id must not erase the Audible link.
        let c = upsert_series(&pool, "The Lord of the Rings", "user", None)
            .await
            .unwrap();
        assert_eq!(a, c);
        let (provider, provider_id): (Option<String>, Option<String>) =
            sqlx::query_as("SELECT provider, provider_id FROM series WHERE id = ?")
                .bind(a)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(provider.as_deref(), Some("audible"));
        assert_eq!(provider_id.as_deref(), Some("B005NF6MIQ"));
    }

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
