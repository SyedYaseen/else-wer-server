use crate::{
    AppState,
    api::{
        api_error::ApiError, auth_extractor::AuthUser, match_meta::try_spawn_metadata_backfill,
        middleware::AdminUser,
    },
    db::libraries::{create_library, delete_library, get_library, list_libraries, update_library},
    file_ops::{book_cover::cover_links, scan_files::scan_files},
    models::libraries::{CreateLibraryDto, UpdateLibraryDto},
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};

// Read is safe to expose broadly (only used for a filter/selector UI); only
// mutation is admin-gated below.
pub async fn list_libraries_handler(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
) -> Result<impl IntoResponse, ApiError> {
    let libraries = list_libraries(&state.db_pool).await?;
    Ok(Json(libraries))
}

pub async fn create_library_handler(
    AdminUser(_claims): AdminUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateLibraryDto>,
) -> Result<impl IntoResponse, ApiError> {
    if payload.name.trim().is_empty() || payload.path.trim().is_empty() {
        return Err(ApiError::BadRequest("Provide both name and path".into()));
    }
    let library = create_library(&state.db_pool, &payload).await?;
    Ok((StatusCode::CREATED, Json(library)))
}

pub async fn update_library_handler(
    AdminUser(_claims): AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateLibraryDto>,
) -> Result<impl IntoResponse, ApiError> {
    let library = update_library(&state.db_pool, id, &payload).await?;
    Ok(Json(library))
}

pub async fn delete_library_handler(
    AdminUser(_claims): AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, ApiError> {
    let rows_affected = delete_library(&state.db_pool, id).await?;
    if rows_affected == 0 {
        return Err(ApiError::NotFound(format!("No library with id {id}")));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn scan_library_handler(
    AdminUser(_claims): AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, ApiError> {
    let db = &state.db_pool;
    let library = get_library(db, id).await?;

    if !state.scan_guard.try_start_library(id) {
        return Ok((
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "message": "A scan is already running" })),
        ));
    }

    let result = scan_files(library.id, &library.path, db).await;
    state.scan_guard.finish_library(id);
    let files_scanned = result?;

    cover_links(db).await?;
    // Fire-and-forget: fills missing cover/description/series from Audible (1.5s/book).
    try_spawn_metadata_backfill(state.clone());

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "message": "Scan completed successfully",
            "files_scanned": files_scanned,
        })),
    ))
}
