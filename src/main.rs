mod api;
mod config;
mod db;
mod file_ops;
mod models;
mod services;
use crate::{config::Config, services::startup::init_tracing};
use axum::Router;
use dotenv::dotenv;
use services::startup::ensure_admin_user;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::net::TcpListener;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: SqlitePool,
    pub config: Arc<Config>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    init_tracing();

    // tracing_subscriber::registry()
    //     .with(
    //         tracing_subscriber::EnvFilter::try_from_default_env()
    //             .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
    //     )
    //     .with(tracing_subscriber::fmt::layer())
    //     .init();

    let config = Arc::new(Config::from_env().unwrap());
    let db_pool = db::init_db_pool(&config.database_url)
        .await
        .expect("Err connecting to database");

    ensure_admin_user(&db_pool).await.unwrap();

    let state = AppState {
        db_pool: db_pool,
        config: Arc::clone(&config),
    };

    let app = Router::new()
        .nest("/api", api::routes().await)
        .with_state(state);

    let listener = TcpListener::bind(format!("{}:{}", &config.host, &config.port))
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
