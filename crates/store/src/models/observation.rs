use serde::{Deserialize, Serialize};

/// Observation — Sulix 对现实世界的一次结构化观察事件。
///
/// 核心链路：Source → Observation → Evidence → Claim → Signal
/// Observation 不是 Article 的替代品，而是从外部 Source 中提取的认知事件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: i64,
    pub source_type: String,
    pub source_id: String,
    pub title: String,
    pub summary: Option<String>,
    pub content_hash: Option<String>,
    pub observed_at: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewObservation {
    pub source_type: String,
    pub source_id: String,
    pub title: String,
    pub summary: Option<String>,
    pub content_hash: Option<String>,
    pub observed_at: i64,
}
