use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DailyStat {
    pub day: String,
    pub ms_listened: i64,
}

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct FinishedBook {
    pub book_id: i64,
    pub times_finished: i64,
    pub finished_at: DateTime<Utc>,
}
