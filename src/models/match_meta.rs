use serde::{Deserialize, Serialize};

/// One external-provider match candidate for a book. Returned by GET /match_book and
/// sent back (user-chosen) as the POST /match_book body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchCandidate {
    pub title: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub narrator: Option<String>,
    #[serde(default)]
    pub series_name: Option<String>,
    #[serde(default)]
    pub series_sequence: Option<String>,
    #[serde(default)]
    pub year: Option<i64>,
    #[serde(default)]
    pub asin: Option<String>,
    #[serde(default)]
    pub cover_url: Option<String>,
    /// Similarity of candidate title+author vs the book's, computed server-side on GET.
    #[serde(default)]
    pub confidence: f64,
}

/// POST /match_book body: the candidate the user picked, plus whether the book's
/// display title/author should be overwritten with the candidate's.
#[derive(Debug, Deserialize)]
pub struct ApplyMatchDto {
    #[serde(flatten)]
    pub candidate: MatchCandidate,
    #[serde(default)]
    pub apply_title_author: bool,
}
