use serde::{Deserialize, Serialize};

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
