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
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: SqlitePool,
    pub config: Arc<Config>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    init_tracing();

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
        .with_state(state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        );

    let listener = TcpListener::bind(format!("{}:{}", &config.host, &config.port))
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
