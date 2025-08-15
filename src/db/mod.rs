pub mod audiobooks;
pub mod sync;
pub mod user;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Error as SqlxError, SqlitePool};
use std::str::FromStr;

use crate::api::api_error::ApiError;
use crate::file_ops::file_ops::scan_for_audiobooks;

pub type DbPool = SqlitePool;

pub async fn init_db_pool(db_url: &str) -> Result<DbPool, SqlxError> {
    let connection_options = SqliteConnectOptions::from_str(db_url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connection_options)
        .await?;

    // Run migrations
    sqlx::migrate!().run(&pool).await?;

    Ok(pool)
}

pub async fn cleanup(db: &SqlitePool) -> Result<(), ApiError> {
    let _ = sqlx::query!(
        r#"
        DELETE FROM files
        "#
    )
    .execute(db)
    .await?;

    let _res = sqlx::query!(
        r#"
        DELETE FROM audiobooks
        "#
    )
    .execute(db)
    .await?;

    tracing::warn!("Deleted AUDIOBOOKS and FILES");
    scan_for_audiobooks("data", db).await?;
    tracing::warn!("Completed scan of AUDIOBOOKS and FILES");
    Ok(())
}
