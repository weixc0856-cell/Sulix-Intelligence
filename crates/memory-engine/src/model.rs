//! Memory bounded context — domain persistence records.
//!
//! Owned here so the engine depends on no `store` types; the infrastructure
//! adapter maps between these and the D1 rows.

/// Promotion score — calculated in the evaluator.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PromotionScore {
    pub confidence: f32,
    pub recurrence: f32,
    pub impact: f32,
    pub evidence: f32,
    pub stability: f32,
    pub total: f32,
}

/// Input for persisting a newly promoted memory.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NewMemory {
    pub memory_type: String,
    pub memory_origin: String,
    pub statement: String,
    pub confidence: f64,
    pub stability_score: Option<f64>,
    /// JSON-encoded source lineage.
    pub memory_sources: Option<String>,
    pub artifact_key: Option<String>,
    pub status: String,
}

/// A reflection-derived event the memory engine may consolidate into a memory.
///
/// `aggregate_id` is the source reflection id (e.g. `REF-000123`).
#[derive(Debug, Clone)]
pub struct MemoryEventRef {
    pub event_id: String,
    pub aggregate_id: String,
    pub object_key: String,
    pub occurred_at: i64,
}
