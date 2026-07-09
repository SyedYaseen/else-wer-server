use crate::{
    AppState,
    api::{api_error::ApiError, auth_extractor::AuthUser},
    db::audiobooks::{get_book, list_all_books, update_cover_art, update_description},
    db::series::{assign_books_to_series, get_book_title_author, set_book_match, upsert_series},
    file_ops::{book_cover::download_cover, book_meta_file::write_book_meta_json, meta_cleanup::fold_key},
    models::match_meta::{ApplyMatchDto, AssignSeriesDto, MatchCandidate},
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

// Only auto-attach a bulk-backfilled cover when the candidate is a confident match —
// there's no per-book human confirmation in this path, unlike the manual match sheet.
const MIN_BACKFILL_CONFIDENCE: f64 = 0.5;
// Space out Audible search calls during a bulk backfill to avoid rate limiting.
const BACKFILL_DELAY: std::time::Duration = std::time::Duration::from_millis(1500);

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
    let book = get_book(db, book_id).await?;

    let candidate = &payload.candidate;

    let series_id = match candidate.series_name.as_deref() {
        Some(name) if !name.trim().is_empty() => Some(
            upsert_series(db, name.trim(), "audible", candidate.series_asin.as_deref()).await?,
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

    if book.cover_art.is_none() {
        if let Some(cover_url) = candidate.cover_url.as_deref() {
            match download_cover(cover_url, &book).await {
                Ok(Some(cover_link)) => {
                    if let Err(e) = update_cover_art(db, book_id, cover_link).await {
                        tracing::warn!("Failed to save cover_art for book {book_id}: {e}");
                    }
                }
                Ok(None) => {}
                Err(e) => tracing::warn!("Failed to download cover art for book {book_id}: {e}"),
            }
        }
    }

    if let Some(description) = candidate.description.as_deref() {
        if let Err(e) = update_description(db, book_id, description).await {
            tracing::warn!("Failed to save description for book {book_id}: {e}");
        }
    }

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

// Manual series reconcile: link the given books to one series (existing id, or
// create-or-reuse by name), or remove them from any series when neither is given.
// Sets series_locked either way so the automatic pass never overrides the user.
pub async fn assign_series(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
    Json(payload): Json<AssignSeriesDto>,
) -> Result<impl IntoResponse, ApiError> {
    let db = &state.db_pool;
    if payload.books.is_empty() {
        return Err(ApiError::BadRequest("books must not be empty".into()));
    }

    let series_id = match (
        payload.series_id,
        payload.series_name.as_deref().map(str::trim),
    ) {
        (Some(id), _) => {
            // 404 before writing anything.
            let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM series WHERE id = ?")
                .bind(id)
                .fetch_optional(db)
                .await?;
            Some(exists.ok_or_else(|| ApiError::NotFound(format!("No series with id {id}")))?)
        }
        (None, Some(name)) if !name.is_empty() => {
            Some(upsert_series(db, name, "user", None).await?)
        }
        _ => None, // clear: remove the books from their series
    };

    let books: Vec<(i64, Option<String>)> = payload
        .books
        .iter()
        .map(|b| {
            // A sequence is meaningless without a series.
            let seq = if series_id.is_some() { b.sequence.clone() } else { None };
            (b.book_id, seq)
        })
        .collect();

    let updated = assign_books_to_series(db, series_id, &books).await?;

    // Persist next to the audio files so the assignment survives a DB wipe.
    for (book_id, _) in &books {
        if let Err(e) = write_book_meta_json(db, *book_id).await {
            tracing::warn!("assign_series: metadata.json write failed for book {book_id}: {e}");
        }
    }

    Ok((
        StatusCode::OK,
        Json(json!({ "series_id": series_id, "updated": updated })),
    ))
}

#[derive(serde::Serialize)]
pub struct BackfillStats {
    pub checked: usize,
    pub updated: usize,
    pub skipped: usize,
}

// Find every book missing cover art, description, or a (non-user-locked) series link,
// search Audible once per book, and fill only the missing fields on a confident match.
// Sequential with a delay between books to avoid rate limiting.
// Never touches title/author/plain-text series — those are scan-bucketing identity.
pub(crate) async fn run_metadata_backfill(
    db: &sqlx::SqlitePool,
) -> Result<BackfillStats, ApiError> {
    let books: Vec<_> = list_all_books(db)
        .await?
        .into_iter()
        .filter(|b| {
            b.cover_art.is_none()
                || b.description.is_none()
                || (b.series_id.is_none() && !b.series_locked)
        })
        .collect();

    let mut updated = 0;
    let mut skipped = 0;
    let checked = books.len();

    for book in &books {
        let result = async {
            let mut candidates =
                audible::search_products(&book.title, Some(&book.author)).await?;
            for c in &mut candidates {
                c.confidence = candidate_confidence(c, &book.title, &book.author);
            }
            candidates.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
            let best = candidates
                .into_iter()
                .find(|c| c.confidence > MIN_BACKFILL_CONFIDENCE);

            let Some(c) = best else {
                return Ok(false);
            };
            let mut filled = false;

            if book.cover_art.is_none()
                && let Some(cover_url) = c.cover_url.as_deref()
                && let Some(link) = download_cover(cover_url, book).await?
            {
                update_cover_art(db, book.id, link).await?;
                filled = true;
            }

            if book.description.is_none()
                && let Some(description) = c.description.as_deref()
            {
                update_description(db, book.id, description).await?;
                filled = true;
            }

            if book.series_id.is_none()
                && !book.series_locked
                && let Some(name) = c
                    .series_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|n| !n.is_empty())
            {
                let sid = upsert_series(db, name, "audible", c.series_asin.as_deref()).await?;
                set_book_match(
                    db,
                    book.id,
                    Some(sid),
                    c.series_sequence.as_deref(),
                    c.asin.as_deref(),
                    None,
                )
                .await?;
                filled = true;
            }

            if filled {
                // Persist next to the audio files so the series link survives a DB wipe.
                write_book_meta_json(db, book.id).await?;
            }
            Ok::<bool, ApiError>(filled)
        }
        .await;

        match result {
            Ok(true) => updated += 1,
            Ok(false) => skipped += 1,
            Err(e) => {
                tracing::warn!("metadata backfill: failed for book {}: {e}", book.id);
                skipped += 1;
            }
        }

        tokio::time::sleep(BACKFILL_DELAY).await;
    }

    Ok(BackfillStats {
        checked,
        updated,
        skipped,
    })
}

/// Spawn a backfill pass unless one is already in flight; returns whether it started.
/// Detached task, so callers (scan endpoints, manual trigger) return immediately and
/// a client disconnect can't cancel the run mid-way; stats only go to the log.
pub(crate) fn try_spawn_metadata_backfill(state: AppState) -> bool {
    use std::sync::atomic::Ordering;
    if state
        .backfill_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        tracing::info!("metadata backfill already running, skipping this trigger");
        return false;
    }
    tokio::spawn(async move {
        match run_metadata_backfill(&state.db_pool).await {
            Ok(stats) => tracing::info!(
                checked = stats.checked,
                updated = stats.updated,
                skipped = stats.skipped,
                "metadata backfill finished"
            ),
            Err(e) => tracing::warn!("metadata backfill failed: {e}"),
        }
        state.backfill_running.store(false, Ordering::SeqCst);
    });
    true
}

pub async fn backfill_metadata(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
) -> Result<impl IntoResponse, ApiError> {
    if try_spawn_metadata_backfill(state) {
        Ok((
            StatusCode::ACCEPTED,
            Json(json!({ "message": "metadata backfill started" })),
        ))
    } else {
        Ok((
            StatusCode::CONFLICT,
            Json(json!({ "message": "metadata backfill already running" })),
        ))
    }
}
