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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RuleEntry {
    pub id: i64,
    pub name: String,
    pub rule_json: String,
    pub audience_tag: String,
    pub enabled: bool,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignalStrategy {
    pub id: i64,
    pub name: String,
    pub signal_type: Option<String>,
    pub rule_json: String,
    pub audience_tag: String,
    #[serde(default)]
    pub score_delta: f64,
    pub enabled: bool,
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

// ---- Preview types ----

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PreviewRequest {
    pub condition: serde_json::Value,
    #[serde(default)]
    pub score_delta: f64,
    pub signal_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PreviewMatch {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub published_at: Option<i64>,
    pub feed_name: Option<String>,
    pub score_change: f64,
    pub matched_reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PreviewResult {
    pub total: i64,
    pub matched: i64,
    pub signal_type: Option<String>,
    pub items: Vec<PreviewMatch>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignalSummary {
    pub signal_type: Option<String>,
    pub strategy_count: i64,
    pub total_score_delta: f64,
    pub avg_score_delta: f64,
    pub enabled_count: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignalEvidence {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub feed_name: Option<String>,
    pub published_at: Option<i64>,
    pub score: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TodaySignal {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub confidence: f64,
    pub evidence_count: i64,
    pub trend: String,
    pub articles: Vec<SignalEvidence>,
}
