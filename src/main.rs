mod api;
mod config;
mod db;
mod file_ops;
mod models;
mod services;
use crate::{
    config::Config,
    db::cleanup,
    file_ops::file_ops::scan_for_audiobooks,
    services::startup::{init_logging, scan_files_startup, shutdown_signal},
};
use axum::{
    Router,
    http::{self, HeaderValue, Method, Request},
};
use dotenv::dotenv;
use services::startup::ensure_admin_user;
use sqlx::SqlitePool;
use std::{net::SocketAddr, sync::Arc};
use tower_http::cors::{Any, CorsLayer};
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::{Level, Span, info};

#[derive(Clone)]
pub struct AppState {
    pub db_pool: SqlitePool,
    pub config: Arc<Config>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    init_logging();

    let config = Arc::new(Config::from_env().unwrap());
    let db_pool = db::init_db_pool(&config.database_url)
        .await
        .expect("Err connecting to database");

    // let _ = cleanup(&db_pool).await;
    ensure_admin_user(&db_pool).await.unwrap();
    let _ = scan_files_startup(&config.audiobook_location, &db_pool).await;

    let state = AppState {
        db_pool: db_pool,
        config: Arc::clone(&config),
    };

    let cors = CorsLayer::new()
        // .allow_origin("http://localhost:3001".parse::<HeaderValue>().unwrap())
        .allow_origin(Any) // allows all origins
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([http::header::CONTENT_TYPE, http::header::AUTHORIZATION]);
    let app = Router::new()
        .nest("/api", api::routes().await)
        .with_state(state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|req: &Request<_>| {
                    let req_id = req
                        .headers()
                        .get("x-request-id")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("-");
                    let method = req.method().clone();
                    let uri = req.uri().clone();
                    let version = format!("{:?}", req.version());

                    // Create a span that will wrap the whole request
                    tracing::span!(
                        Level::INFO,
                        "http.request",
                        request_id = %req_id,
                        method = %method,
                        uri = %uri,
                        version = %version,
                    )
                })
                .on_request(|_req: &Request<_>, _span: &Span| {
                    tracing::info!(
                        target: "http",
                        "Request Start"
                    );
                })
                .on_response(
                    |res: &axum::http::Response<_>, latency: std::time::Duration, _span: &Span| {
                        tracing::info!(
                            target: "http",
                            status = res.status().as_u16(),
                            latency_ms = %latency.as_millis(),
                            "Request end"
                        );
                    },
                )
                .on_failure(
                    // Logs errors like timeouts / panics during reading body, etc.
                    tower_http::trace::DefaultOnFailure::new().level(Level::ERROR),
                ),
        )
        .layer(SetRequestIdLayer::new(
            http::header::HeaderName::from_static("x-request-id"),
            MakeRequestUuid,
        ))
        // This propagates it back to the response
        .layer(PropagateRequestIdLayer::new(
            http::header::HeaderName::from_static("x-request-id"),
        ))
        .layer(cors);

    let addr: SocketAddr = "0.0.0.0:3000".parse().unwrap();
    info!(%addr, "listening");
    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    Ok(())
}
