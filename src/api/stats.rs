use crate::{
    AppState,
    api::{api_error::ApiError, auth_extractor::AuthUser},
    db::stats::{daily_totals, list_finished_books},
};
use axum::{
    Json,
    extract::{Query, State},
    response::IntoResponse,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct DailyStatsQuery {
    days: Option<i64>,
}

pub async fn get_daily_stats(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(params): Query<DailyStatsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let days = params.days.unwrap_or(30).clamp(1, 365);
    let rows = daily_totals(&state.db_pool, claims.sub, days).await?;
    Ok(Json(rows))
}

pub async fn get_finished_books(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<impl IntoResponse, ApiError> {
    let rows = list_finished_books(&state.db_pool, claims.sub).await?;
    Ok(Json(rows))
}
