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
/// carries the fields the discovery sources read, plus `entity_type` retained
/// for the candidate-quality reference check (`candidate.rs`). Factor sub-scores
/// (volume/diversity/quality/velocity/novelty) are computed by the SQL query
/// into `score` and are not read here, so they are dropped at the boundary.
#[derive(Debug, Clone)]
pub struct EntityCandidate {
    pub entity_id: i64,
    pub entity_name: String,
    pub entity_type: String,
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

// ===== Read models (SignalQuery boundary) =====
//
// Mirrors of the store's signal read-model DTOs. The nested types that are
// serialised to the frontend (`SignalDetail` + its tree) keep the exact serde
// shape of their `store` counterparts so the query JSON is unchanged; the
// intermediate listing/event records are trimmed to what the read-model code
// actually reads.

/// Health component scores backing a thread's health score.
/// Same shape as `store::HealthComponents`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthComponents {
    pub volume: f64,
    pub diversity: f64,
    pub quality: f64,
    pub velocity: f64,
    pub persistence: f64,
}

/// Detailed health score for a signal thread.
/// Same shape as `store::SignalHealthDetail2`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalHealthDetail2 {
    pub score: f64,
    pub components: HealthComponents,
}

/// Lightweight entity reference.
/// Same shape as `store::EntitySignalRef`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntitySignalRef {
    pub id: i64,
    pub name: String,
    pub entity_type: String,
}

/// A related signal thread reference.
/// Same shape as `store::RelatedSignalRef`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelatedSignalRef {
    pub id: i64,
    pub title: String,
    pub status: String,
    pub health_score: f64,
}

/// A single timeline event (instance or stored event).
/// Same shape as `store::SignalTimelineEvent`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalTimelineEvent {
    pub timestamp: i64,
    pub event_type: String,
    pub score: f64,
    pub article_count: i64,
    pub description: String,
}

/// Rule-based "Why This Matters" analysis.
/// Same shape as `store::SignalAnalysis`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalAnalysis {
    pub why_it_matters: String,
    pub impact: String,
    pub confidence_reason: String,
}

/// Full read model for the Signal Detail page — thread metadata, health,
/// timeline, evidence, entities and related signals.
/// Same shape as `store::SignalDetail`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalDetail {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub trend: String,
    pub health: SignalHealthDetail2,
    pub anchor_entity: Option<EntitySignalRef>,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
    pub timeline: Vec<SignalTimelineEvent>,
    pub evidence_top: Vec<BriefArticle>,
    pub related_entities: Vec<RelatedEntityRef>,
    pub related_signals: Vec<RelatedSignalRef>,
    /// Rule-based "Why This Matters" analysis.
    pub analysis: Option<SignalAnalysis>,
}

/// A stored D1 `signal_events` row (legacy timeline fallback). Trimmed from
/// `store::SignalEvent` to the fields the detail query reads when parsing a
/// stored event into a timeline entry.
#[derive(Debug, Clone)]
pub struct SignalEventRecord {
    pub event_type: String,
    pub payload: Option<String>,
    pub created_at: i64,
}

/// Filter for listing signal threads.
/// Same shape as `store::SignalThreadFilter`.
#[derive(Debug, Clone)]
pub struct SignalThreadFilter {
    pub statuses: Vec<String>,
    pub limit: u32,
    pub min_score: f64,
}

/// Instance timeline head as read by the listing projection (only timestamps).
#[derive(Debug, Clone)]
pub struct SignalInstanceMoment {
    pub generated_at: i64,
}

/// A signal-thread listing row. Trimmed projection of `store::SignalBriefInput`
/// to the fields the entity-thread query reads while building its summaries.
#[derive(Debug, Clone)]
pub struct SignalThreadRow {
    pub thread_id: i64,
    pub signal_key: String,
    pub anchor_entity: Option<String>,
    pub title: String,
    pub status: String,
    pub health_score: f64,
    pub trend: String,
    pub current_score: f64,
    pub cumulative_article_count: i64,
    pub instances: Vec<SignalInstanceMoment>,
}
