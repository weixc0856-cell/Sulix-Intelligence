//! Dependency-boundary ports for signal-engine (decoupling P3 Round 2).
//!
//! These are the edges through which the engine reaches the outside world:
//! signal persistence, candidate discovery, read-model query, the event log,
//! and semantic ANN retrieval.
//!
//! ⚠️ Temporary, use-case-specific boundary — NOT a public shared-kernel /
//! intelligence-domain capability. signal-engine is a deprecated crate
//! (`lib.rs` banner; removal scheduled Sprint 6.2E+). When the crate is
//! deleted, these ports and their infrastructure adapters are removed together.

use async_trait::async_trait;

use crate::error::SignalError;
use crate::models::{DiscoveryMethod, EmbeddedArticle, EntityCandidate, SignalUpsertResult};

/// An event appended to the signal event log.
#[derive(Debug, Clone)]
pub struct SignalEvent {
    /// e.g. `SignalScoreChanged` / `SignalCreated`.
    pub event_type: String,
    /// Aggregate this event belongs to, e.g. `SIG-{thread_id:06}`.
    pub aggregate_id: String,
    pub payload: serde_json::Value,
    pub occurred_at: i64,
}

/// Append/read the durable signal event log (R2 event archive in production).
///
/// The per-write `sequence` (a per-run counter in the engine) is event-id
/// *generation* metadata, not domain semantics — the read side never consumes
/// it — so it is passed to [`SignalEventLog::append`] rather than carried on
/// [`SignalEvent`]. The adapter derives the stored `event_id` from
/// `(occurred_at, sequence)`, mirroring the legacy `evt_{ts}_{seq}` scheme.
#[async_trait(?Send)]
pub trait SignalEventLog {
    async fn append(&self, event: &SignalEvent, sequence: u64) -> Result<(), SignalError>;
    async fn load(&self, aggregate_id: &str, limit: u32) -> Result<Vec<SignalEvent>, SignalError>;
}

/// A match returned by semantic ANN search.
///
/// `vector_id` is the raw stored-vector identifier returned by the index
/// (namespaced, e.g. `article-42`). Interpreting it further — e.g. recovering
/// an article id — is the domain consumer's concern, not the adapter's, so the
/// infrastructure field name (`id`) never leaks into the domain port.
#[derive(Debug, Clone)]
pub struct SimilarArticle {
    pub vector_id: String,
    pub score: f64,
}

/// Semantic ANN retrieval boundary — used by [`crate::source::SemanticDiscoverySource`].
#[async_trait(?Send)]
pub trait SemanticQuery {
    /// Find the nearest stored vectors to `vector_id` in the semantic index.
    async fn find_similar(
        &self,
        vector_id: &str,
        top_k: u32,
        min_score: f64,
    ) -> Result<Vec<SimilarArticle>, SignalError>;
}

/// Signal write orchestration boundary — the `run()` write path.
///
/// Wraps the store's thread upsert / instance append / lifecycle calls so the
/// engine no longer reaches `StoreBackend` directly.
#[async_trait(?Send)]
pub trait SignalPersistence {
    /// Upsert a signal thread by its `signal_key`; reports whether it was
    /// created or merely updated.
    async fn upsert_signal_thread(
        &self,
        signal_key: &str,
        anchor_entity_id: Option<i64>,
        title: &str,
        status: &str,
        discovery_method: &DiscoveryMethod,
        discovery_score: Option<f64>,
    ) -> Result<SignalUpsertResult, SignalError>;

    /// Get the latest instance's `(score, trend)` for change dedup.
    async fn latest_instance_fingerprint(&self, thread_id: i64) -> Result<Option<(f64, String)>, SignalError>;

    /// Append a signal instance snapshot.
    #[allow(clippy::too_many_arguments)]
    async fn append_signal_instance(
        &self,
        thread_id: i64,
        score: f64,
        impact: &str,
        trend: &str,
        article_count: i64,
        source_count: i64,
        avg_score: f64,
        entity_id: i64,
    ) -> Result<i64, SignalError>;

    /// Update signal lifecycle (active → decaying → resolved → archived).
    async fn update_signal_lifecycle(&self, now: i64) -> Result<(), SignalError>;
}

/// Candidate-discovery boundary — the source read path.
///
/// Wraps the store's entity-candidate + recent-embedded-article queries so the
/// discovery sources no longer reach `StoreBackend` directly.
#[async_trait(?Send)]
pub trait SignalDiscovery {
    /// Entity-anchored signal candidates with quality filters.
    async fn entity_signal_candidates(
        &self,
        now: i64,
        days: i64,
        limit: u32,
        min_entity_articles: u32,
        min_sources: u32,
    ) -> Result<Vec<EntityCandidate>, SignalError>;

    /// Recent articles that carry a Vectorize embedding (ANN anchor set).
    async fn recent_embedded_articles(
        &self,
        now: i64,
        days: i64,
        limit: u32,
    ) -> Result<Vec<EmbeddedArticle>, SignalError>;
}
