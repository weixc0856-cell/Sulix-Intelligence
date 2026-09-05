use serde::{Deserialize, Serialize};

/// Observation — Sulix 对现实世界的一次结构化观察事件。
///
/// 核心链路：Source → Observation → Evidence → Claim → Signal
/// Observation 不是 Article 的替代品，而是从外部 Source 中提取的认知事件。
///
/// Sprint 5.6: 增加 url / article_id / registry_source_id 以支持 lineage tracing。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: i64,
    pub source_type: String,
    pub source_id: String,
    pub title: String,
    pub summary: Option<String>,
    pub content_hash: Option<String>,
    pub url: Option<String>,             // Sprint 5.6
    pub article_id: Option<i64>,         // Sprint 5.6: FK to articles
    pub registry_source_id: Option<i64>, // Sprint 5.6: FK to sources
    pub observed_at: i64,
    pub created_at: i64,
}

/// Input for creating an Observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewObservation {
    pub source_type: String,
    pub source_id: String,
    pub title: String,
    pub summary: Option<String>,
    pub content_hash: Option<String>,
    pub url: Option<String>,             // Sprint 5.6
    pub article_id: Option<i64>,         // Sprint 5.6
    pub registry_source_id: Option<i64>, // Sprint 5.6
    pub observed_at: i64,
}
