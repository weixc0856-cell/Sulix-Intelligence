//! Owned records the context-engine reads/writes through its persistence port.
//!
//! These are projections over store rows — only the fields the retriever and
//! snapshot writer actually consume. The `infrastructure` adapters map store
//! DTOs onto these so this crate never names a store type.

/// Projection of a decision for context retrieval.
#[derive(Debug, Clone)]
pub struct DecisionRecord {
    pub id: i64,
    pub title: String,
    pub decision_type: String,
    pub status: String,
    pub confidence: f64,
}

/// Projection of a memory for context retrieval.
#[derive(Debug, Clone)]
pub struct MemoryRecord {
    pub id: i64,
    pub statement: String,
    pub memory_type: String,
    pub confidence: f64,
    pub usage_count: i64,
}

/// A context snapshot to persist. Drops the store model's `object_key` /
/// `object_size` fields: the R2 artifact path was dead code (all callers passed
/// no object store), so the snapshot is D1-only.
#[derive(Debug, Clone)]
pub struct NewContextSnapshot {
    pub id: String,
    pub query: String,
    pub intent: String,
    pub domain: Option<String>,
    pub context_json: String,
    pub evidence_refs: Option<String>,
    pub confidence: f64,
    pub user_scope: Option<String>,
}
