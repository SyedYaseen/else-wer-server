mod api;
mod config;
mod db;
mod file_ops;
mod models;
mod utils;
use crate::config::Config;
use axum::{Router, extract::State, http::StatusCode};
use dotenv::dotenv;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::ensure_admin_user::ensure_admin_user;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: SqlitePool,
    pub config: Arc<Config>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

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

struct DatabaseConnection(sqlx::pool::PoolConnection<sqlx::Sqlite>);

async fn using_connection_extractor(
    DatabaseConnection(mut conn): DatabaseConnection,
) -> Result<String, (StatusCode, String)> {
    sqlx::query_scalar("select 'hello world from pg'")
        .fetch_one(&mut *conn)
        .await
        .map_err(internal_error)
}

async fn using_connection_pool_extractor(
    State(state): State<AppState>,
) -> Result<String, (StatusCode, String)> {
    sqlx::query_scalar("select 'hello world from pg'")
        .fetch_one(&state.db_pool)
        .await
        .map_err(internal_error)
}

fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
