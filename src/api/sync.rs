use crate::{
    AppState,
    api::{api_error::ApiError, auth_extractor::AuthUser},
    db::stats::{add_listened_ms, is_book_fully_complete, record_finish_transition, sanitize_delta},
    db::sync::{
        get_progress_by_bookid, get_progress_by_fileid, list_inprogress_db, upsert_progress,
    },
    models::user::{Progress, ProgressUpdate},
};
use Result::Ok;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;

pub async fn list_inprogress(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> impl IntoResponse {
    match list_inprogress_db(&state.db_pool, claims.sub).await {
        Ok(progress) => Json(progress).into_response(),
        Err(e) => {
            tracing::error!("Err fetching inp progress books: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "DB error").into_response()
        }
    }
}

pub async fn get_file_progress(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path((book_id, file_id)): Path<(i64, i64)>,
) -> impl IntoResponse {
    match get_progress_by_fileid(&state.db_pool, claims.sub, book_id, file_id).await {
        Ok(Some(progress)) => Json(progress).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "Progress not found").into_response(),
        Err(e) => {
            tracing::error!("DB error fetching progress: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "DB error").into_response()
        }
    }
}

pub async fn get_book_progress(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(book_id): Path<i64>,
) -> impl IntoResponse {
    match get_progress_by_bookid(&state.db_pool, claims.sub, book_id).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => {
            tracing::error!("DB error fetching progress: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "DB error").into_response()
        }
    }
}

pub async fn update_progress(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser, // this needs to be in middle. Axum wants in this order
    Json(payload): Json<ProgressUpdate>,
) -> Result<impl IntoResponse, ApiError> {
    tracing::debug!("Incoming update payload: {:#?}", payload);

    let was_complete_before = is_book_fully_complete(&state.db_pool, claims.sub, payload.book_id)
        .await
        .unwrap_or(false);

    upsert_progress(&state.db_pool, claims.sub, &payload)
        .await
        .map_err(|e| {
            tracing::error!("Upsert Error: {e}");
            if e.as_database_error()
                .is_some_and(|de| de.is_foreign_key_violation())
            {
                return ApiError::NotFound("Book or file not found".into());
            }
            ApiError::Internal("Upsert failed".into())
        })?;

    tracing::debug!("Upsert succeeded");

    // Best-effort: stats/finish tracking never fails the progress save itself.
    if let Some(delta) = payload.listened_delta_ms.and_then(sanitize_delta) {
        let today = Utc::now().date_naive();
        if let Err(e) = add_listened_ms(&state.db_pool, claims.sub, payload.book_id, today, delta).await {
            tracing::error!("Failed to record listening stats: {e}");
        }
    }

    if !was_complete_before {
        match is_book_fully_complete(&state.db_pool, claims.sub, payload.book_id).await {
            Ok(true) => {
                if let Err(e) = record_finish_transition(&state.db_pool, claims.sub, payload.book_id).await {
                    tracing::error!("Failed to record finish transition: {e}");
                }
            }
            Ok(false) => {}
            Err(e) => tracing::error!("Failed to check book completion: {e}"),
        }
    }

    Ok(StatusCode::NO_CONTENT)
}
