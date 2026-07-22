//! Domain types for the D1 access layer.  Every other crate imports these
//! from `store` rather than defining its own structs, keeping the schema
//! contract in one place.

use serde::{Deserialize, Serialize};

// ---- Error ----

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("d1 error: {0}")]
    D1(String),
}

impl From<worker::Error> for StoreError {
    fn from(e: worker::Error) -> Self {
        StoreError::D1(e.to_string())
    }
}

// ---- Entities ----

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Feed {
    pub id: i64,
    pub url: String,
    pub title: Option<String>,
    pub category: Option<String>,
    pub fetch_interval_sec: i64,
    pub last_fetched_at: Option<i64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub status: String,
    pub extraction_level: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewArticle {
    pub feed_id: i64,
    pub guid: String,
    pub title: String,
    pub url: Option<String>,
    pub published_at: Option<i64>,
    pub raw_content_r2_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Article {
    pub id: i64,
    pub feed_id: i64,
    pub guid: String,
    pub title: String,
    pub url: Option<String>,
    pub published_at: Option<i64>,
    pub ai_summary: String,
    pub ai_tags: Option<String>,
    pub score: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PendingArticle {
    pub id: i64,
    pub feed_id: i64,
    pub guid: String,
    pub title: String,
    pub url: Option<String>,
    pub published_at: Option<i64>,
    pub ai_summary: String,
    pub ai_tags: Option<String>,
    pub score: f64,
    pub raw_content_r2_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ArticleDetail {
    pub id: i64,
    pub feed_id: i64,
    pub feed_name: Option<String>,
    pub guid: String,
    pub title: String,
    pub url: Option<String>,
    pub published_at: Option<i64>,
    pub ai_summary: String,
    pub ai_tags: Option<String>,
    pub score: f64,
}

// ---- View models / query results ----

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeedStats {
    pub id: i64,
    pub title: Option<String>,
    pub url: String,
    pub category: Option<String>,
    pub status: String,
    pub last_fetched_at: Option<i64>,
    pub article_count: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScoreDist {
    pub top: i64,
    pub medium: i64,
    pub low: i64,
    pub unscored: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DayCount {
    pub day: String,
    pub cnt: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HealthStats {
    pub feed_count: i64,
    pub active_feed_count: i64,
    pub article_count: i64,
    pub last_cron_run_at: Option<i64>,
}
