mod api;
mod config;
mod db;
mod file_ops;
mod models;
mod services;
use crate::{
    config::Config,
    services::scan_guard::ScanGuard,
    services::startup::{init_logging, scan_files_startup, shutdown_signal},
};
use axum::{
    Router,
    http::{self, Method, Request},
};
use dotenv::dotenv;
use services::startup::{ensure_admin_user, ensure_default_library};
use sqlx::SqlitePool;
use std::{net::SocketAddr, sync::Arc};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::{Level, Span, info};

#[derive(Clone)]
pub struct AppState {
    pub db_pool: SqlitePool,
    pub config: Arc<Config>,
    // Guards the bulk metadata backfill: one pass at a time across manual + post-scan triggers.
    pub backfill_running: Arc<std::sync::atomic::AtomicBool>,
    // Guards scan triggers (manual/upload/lazy/per-library): serializes a full scan
    // against any per-library scan, but allows different libraries to scan concurrently.
    pub scan_guard: Arc<ScanGuard>,
    pub rate_limiter: Arc<api::rate_limit::LoginRateLimiter>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    init_logging();

    let config = Arc::new(Config::from_env().unwrap());
    let db_pool = db::init_db_pool(&config.database_url)
        .await
        .expect("Err connecting to database");

    ensure_admin_user(&db_pool).await.unwrap();
    ensure_default_library(&db_pool, &config.audiobook_location)
        .await
        .unwrap();
    let _ = scan_files_startup(&config.audiobook_location, &db_pool).await;

    let state = AppState {
        db_pool: db_pool,
        config: Arc::clone(&config),
        backfill_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        scan_guard: Arc::new(ScanGuard::new()),
        rate_limiter: Arc::new(api::rate_limit::LoginRateLimiter::new()),
    };

    // Empty by default: same-origin requests from the embedded PWA don't need CORS
    // headers at all, so an empty allowlist means no cross-origin site can replay a
    // stolen bearer token. Set CORS_ALLOWED_ORIGINS only when the frontend is served
    // from a different origin (e.g. a separate app/site domain).
    let allowed_origins: Vec<http::HeaderValue> = config
        .cors_allowed_origins
        .iter()
        .filter_map(|o| o.parse().ok())
        .collect();
    let cors = CorsLayer::new()
        .allow_origin(allowed_origins)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([http::header::CONTENT_TYPE, http::header::AUTHORIZATION]);
    // Serves the src/ui static build (same-origin) at every path not under
    // /api; unmatched routes fall back to index.html so react-router's client-side routes work.
    let pwa_index = format!("{}/index.html", config.pwa_dist_location);
    let spa_service = ServeDir::new(&config.pwa_dist_location)
        .not_found_service(ServeFile::new(pwa_index));

    let app = Router::new()
        .nest("/api", api::routes().await)
        .fallback_service(spa_service)
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

    // lookup_host so HOST can be a hostname (e.g. "localhost"), not only an IP literal.
    let addr: SocketAddr = tokio::net::lookup_host((config.host.as_str(), config.port))
        .await?
        .next()
        .ok_or_else(|| anyhow::anyhow!("HOST '{}' resolved to no address", config.host))?;
    info!(%addr, "listening");
    axum::serve(
        tokio::net::TcpListener::bind(addr).await.unwrap(),
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .unwrap();

    Ok(())
}
