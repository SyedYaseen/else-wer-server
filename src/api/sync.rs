use crate::{
    AppState,
    api::{api_error::ApiError, auth_extractor::AuthUser},
    db::sync::{get_progress_by_bookid, get_progress_by_fileid, upsert_progress},
    models::user::ProgressUpdate,
};
use Result::Ok;
use axum::{
    Json,
    extract::{FromRequestParts, Path, State},
    http::StatusCode,
    response::IntoResponse,
};

pub async fn get_file_progress(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path((book_id, file_id)): Path<(i64, i64)>,
) -> impl IntoResponse {
    match get_progress_by_fileid(&state.db_pool, claims.sub, book_id, file_id).await {
        Ok(Some(progress)) => Json(progress).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "Progress not found").into_response(),
        Err(e) => {
            eprintln!("DB error fetching progress: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "DB error").into_response()
        }
    }
}

pub async fn get_book_progress(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path((book_id)): Path<(i64)>,
) -> impl IntoResponse {
    match get_progress_by_bookid(&state.db_pool, claims.sub, book_id).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => {
            eprintln!("DB error fetching progress: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "DB error").into_response()
        }
    }
}

pub async fn update_progress(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser, // this needs to be in middle. Axum wants in this order
    Json(payload): Json<ProgressUpdate>,
) -> Result<impl IntoResponse, ApiError> {
    println!("👉 Incoming update payload: {:#?}", payload);

    upsert_progress(&state.db_pool, claims.sub, &payload)
        .await
        .map_err(|e| {
            println!("🚨 Upsert Error: {e}");
            ApiError::InternalServerError("Upsert failed".into())
        })?;

    println!("✅ Upsert succeeded");
    Ok(StatusCode::ACCEPTED)
}
