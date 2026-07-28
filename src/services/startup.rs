use crate::api::api_error::ApiError;
use crate::api::user::save_pwd_hash;
use crate::db::libraries::list_libraries;
use crate::db::user::admin_exists;
use crate::models::user::UserDto;
use sqlx::sqlite::SqlitePool;
use tracing_appender::rolling::{self};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::{fmt, prelude::*};

pub fn init_logging() {
    let file_appender = rolling::daily("logs", "else-wer.log");
    let (non_blocking_file, _guard) = tracing_appender::non_blocking(file_appender);

    let console_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let file_filter = EnvFilter::new("info");

    let stdout_layer = fmt::layer()
        .with_target(false)
        .with_file(true)
        .with_thread_ids(true)
        .with_timer(UtcTime::rfc_3339())
        .with_line_number(true)
        .compact()
        .with_filter(console_filter);

    let file_layer = fmt::layer()
        .json()
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_writer(non_blocking_file)
        .with_filter(file_filter);

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(file_layer)
        .init();

    std::mem::forget(_guard);
}

pub async fn ensure_admin_user(db: &SqlitePool) -> Result<(), ApiError> {
    let admin_exists: i64 = admin_exists(db).await?;

    if admin_exists == 0 {
        let admin = UserDto {
            username: "admin".to_string(),
            password: "admin".to_string(),
            is_admin: true,
            can_organize: true,
        };
        save_pwd_hash(&admin, db).await?;

        tracing::warn!(
            "Admin user created with default credentials username='admin' password='admin' — change this password immediately via PUT /api/user/change_password"
        );
    }

    Ok(())
}

/// One-time migration off the single AUDIOBOOKS_LOCATION env var: seeds a "Default"
/// library from it and backfills any pre-existing audiobooks rows onto it, so an
/// upgrade doesn't orphan data. No-ops once at least one library row exists — after
/// that, libraries are fully DB-managed via the admin API and the env var is unused.
///
/// The insert + backfill run in one transaction so a crash/error between the two
/// can't leave the "Default" library created but pre-existing books never backfilled
/// — the `list_libraries().is_empty()` guard above only re-runs this once, so without
/// the transaction a partial failure would orphan those books' library_id forever.
pub async fn ensure_default_library(db: &SqlitePool, seed_path: &str) -> Result<(), ApiError> {
    if !list_libraries(db).await?.is_empty() {
        return Ok(());
    }

    let mut tx = db.begin().await?;

    let library_id: i64 = sqlx::query_scalar(
        "INSERT INTO libraries (name, path, is_default) VALUES (?1, ?2, 1) RETURNING id",
    )
    .bind("Default")
    .bind(seed_path)
    .fetch_one(&mut *tx)
    .await?;

    let result = sqlx::query!(
        "UPDATE audiobooks SET library_id = ?1 WHERE library_id IS NULL",
        library_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    tracing::info!(
        library_id,
        path = seed_path,
        backfilled_books = result.rows_affected(),
        "seeded Default library"
    );

    Ok(())
}

pub async fn scan_files_startup(path_str: &String, db: &SqlitePool) -> Result<(), ApiError> {
    // info!("Scanning files on {}", path_str);
    // scan_for_audiobooks(path_str, db).await?;
    // scan_files(path_str, db).await?;
    // group_meta_fetch(db).await?;
    // cover_links(db).await?;

    // info!("Completed audiobooks file scan");
    Ok(())
}

pub async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::warn!("shutdown signal received");
}
