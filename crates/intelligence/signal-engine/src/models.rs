//! Owned records on the signal write/discovery boundary (decoupling P3 Round 2).
//!
//! These mirror the shapes of the corresponding `store` DTOs so the
//! infrastructure adapters map 1:1 — no behaviour change. They exist so the
//! domain code (sources, `run()`) no longer imports `store::*` directly.
//!
//! ⚠️ Temporary, use-case-specific records — NOT a public shared-kernel /
//! intelligence-domain model. signal-engine is deprecated (`lib.rs` banner);
//! when the crate is deleted these records and their adapters go with it.

use serde::{Deserialize, Serialize};

/// How a signal thread was discovered — provenance tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscoveryMethod {
    Entity,
    Semantic,
    Hybrid,
}

/// Outcome of a thread upsert — distinguishes first materialisation from an
/// update to an existing thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalMutation {
    Created,
    Updated,
}

/// Result returned by a thread upsert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalUpsertResult {
    pub id: i64,
    pub mutation: SignalMutation,
}

/// A brief article record carried as signal evidence / in a thread summary.
/// Same shape as `store::BriefArticle`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BriefArticle {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub feed_name: Option<String>,
    pub score: f64,
}

/// A lightweight reference to a related entity.
/// Same shape as `store::RelatedEntityRef`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelatedEntityRef {
    pub id: i64,
    pub name: String,
    pub entity_type: String,
    pub relation_type: String,
    /// Human-readable relationship label, e.g. "supplier", "competitor".
    pub relation: Option<String>,
    /// Confidence of the relationship link.
    pub confidence: Option<f64>,
}

/// An entity-driven signal candidate row returned by discovery.
///
/// Trimmed projection of the store's `entity_signal_candidates_filtered` DTO:
/// carries only the fields the discovery sources read. Factor sub-scores
/// (volume/diversity/quality/velocity/novelty) are computed by the SQL query
/// into `score` and are not read here, so they are dropped at the boundary.
#[derive(Debug, Clone)]
pub struct EntityCandidate {
    pub entity_id: i64,
    pub entity_name: String,
    pub score: f64,
    pub trend: String,
    pub article_count: i64,
    pub source_count: i64,
    pub avg_score: f64,
    pub evidence: Vec<BriefArticle>,
    pub related_entity_ids: Vec<i64>,
}

/// A recent embedded article row returned by discovery — the anchor set for
/// semantic ANN search. Trimmed from the store's `ArticleEmbeddingRef` to the
/// two fields the semantic source reads.
#[derive(Debug, Clone)]
pub struct EmbeddedArticle {
    pub article_id: i64,
    pub vector_id: String,
}
