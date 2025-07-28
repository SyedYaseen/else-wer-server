pub mod audiobooks;
pub mod sync;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Error as SqlxError, SqlitePool};
use std::str::FromStr;

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
