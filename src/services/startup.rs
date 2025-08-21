use crate::api::api_error::ApiError;
use crate::api::user::save_pwd_hash;
use crate::db::user::admin_exists;
use crate::file_ops::file_ops::scan_for_audiobooks;
use crate::file_ops::scan_files::scan_files;
use crate::models::user::UserDto;
use axum::extract::path;
use sqlx::sqlite::SqlitePool;
use tracing::info;
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
        };
        save_pwd_hash(&admin, db).await?;

        info!("Admin user created: username='admin'");
    }

    Ok(())
}

pub async fn scan_files_startup(path_str: &String, db: &SqlitePool) -> Result<(), ApiError> {
    info!("Scanning files on {}", path_str);
    // scan_for_audiobooks(path_str, db).await?;
    scan_files(path_str, db).await?;
    info!("Completed audiobooks file scan");
    Ok(())
}

pub async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::warn!("shutdown signal received");
}
