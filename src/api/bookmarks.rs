use crate::{
    AppState,
    api::{api_error::ApiError, auth_extractor::AuthUser},
    db::bookmarks::{create_bookmark, delete_bookmark, list_bookmarks_by_book},
    models::bookmarks::CreateBookmark,
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};

pub async fn create_bookmark_handler(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(payload): Json<CreateBookmark>,
) -> Result<impl IntoResponse, ApiError> {
    let bookmark = create_bookmark(&state.db_pool, claims.sub, &payload)
        .await
        .map_err(|e| {
            if e.as_database_error()
                .is_some_and(|de| de.is_foreign_key_violation())
            {
                return ApiError::NotFound("Book or file not found".into());
            }
            ApiError::Internal("Failed to create bookmark".into())
        })?;

    Ok((StatusCode::CREATED, Json(bookmark)))
}

pub async fn list_bookmarks_handler(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(book_id): Path<i64>,
) -> Result<impl IntoResponse, ApiError> {
    let bookmarks = list_bookmarks_by_book(&state.db_pool, claims.sub, book_id).await?;
    Ok(Json(bookmarks))
}

pub async fn delete_bookmark_handler(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(bookmark_id): Path<i64>,
) -> Result<impl IntoResponse, ApiError> {
    let rows_affected = delete_bookmark(&state.db_pool, claims.sub, bookmark_id).await?;
    if rows_affected == 0 {
        return Err(ApiError::NotFound("Bookmark not found".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}
