use crate::{
    AppState,
    api::{api_error::ApiError, auth_extractor::AuthUser},
    db::series::{get_book_title_author, set_book_match, upsert_series},
    file_ops::{book_meta_file::write_book_meta_json, meta_cleanup::fold_key},
    models::match_meta::{ApplyMatchDto, MatchCandidate},
    services::audible,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;
use strsim::levenshtein;

#[derive(Deserialize)]
pub struct MatchQuery {
    /// Optional search override when the stored title is too mangled to match.
    q: Option<String>,
}

/// Similarity of a candidate against the book's title+author. 1.0 = identical.
fn candidate_confidence(candidate: &MatchCandidate, title: &str, author: &str) -> f64 {
    let a = fold_key(&format!(
        "{} {}",
        candidate.title,
        candidate.author.as_deref().unwrap_or_default()
    ));
    let b = fold_key(&format!("{title} {author}"));
    let longer = a.chars().count().max(b.chars().count());
    if longer == 0 {
        return 0.0;
    }
    1.0 - levenshtein(&a, &b) as f64 / longer as f64
}

// Search the external provider for series/title candidates. Read-only: nothing is
// stored — matching is an explicit user action, never applied on scan.
pub async fn match_book_candidates(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
    Path(book_id): Path<i64>,
    Query(params): Query<MatchQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let db = &state.db_pool;
    let (title, author) = get_book_title_author(db, book_id).await?;

    let (search_title, search_author) = match &params.q {
        Some(q) => (q.as_str(), None),
        None => (title.as_str(), Some(author.as_str())),
    };

    let mut candidates = audible::search_products(search_title, search_author).await?;
    for c in &mut candidates {
        c.confidence = candidate_confidence(c, &title, &author);
    }
    candidates.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));

    Ok((
        StatusCode::OK,
        Json(json!({
            "book_id": book_id,
            "book_title": title,
            "book_author": author,
            "candidates": candidates,
        })),
    ))
}

// Apply the candidate the user picked: upsert the series row and link the book.
pub async fn apply_book_match(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
    Path(book_id): Path<i64>,
    Json(payload): Json<ApplyMatchDto>,
) -> Result<impl IntoResponse, ApiError> {
    let db = &state.db_pool;
    // 404 before writing anything.
    get_book_title_author(db, book_id).await?;

    let candidate = &payload.candidate;

    let series_id = match candidate.series_name.as_deref() {
        Some(name) if !name.trim().is_empty() => Some(
            upsert_series(db, name.trim(), "audible", candidate.asin.as_deref()).await?,
        ),
        _ => None,
    };

    let title_author = if payload.apply_title_author {
        Some((
            candidate.title.as_str(),
            candidate.author.as_deref().unwrap_or_default(),
        ))
    } else {
        None
    };

    set_book_match(
        db,
        book_id,
        series_id,
        candidate.series_sequence.as_deref(),
        candidate.asin.as_deref(),
        title_author,
    )
    .await?;

    // Persist the applied match next to the audio files (portable across DB wipes).
    write_book_meta_json(db, book_id).await?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "message": "Match applied",
            "book_id": book_id,
            "series_id": series_id,
        })),
    ))
}
