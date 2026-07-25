//! `StoreBackend` trait — abstraction over D1 so the pipeline can be
//! unit-tested with a [`MemoryStore`](crate::memory::MemoryStore).
//!
//! MVP scope: only the methods used by the feed processing pipeline.

use async_trait::async_trait;

use crate::{
    ArticleEmbeddingRef, ArtifactEntry, Decision, DecisionEvaluation, DiscoveryMethod, EntityActivitySummary,
    EntityArticle, EntityDetail, EntityRef, EntitySignalCandidate, EntitySummary, Feed, NewArticle, NewArtifact,
    NewDecision, NewDecisionEvaluation, NewOutcomeEvent, OutcomeEvent, RelatedEntity, RelatedEntityRef,
    SignalBriefInput, SignalDetail, SignalEvent, SignalThreadFilter, SignalUpsertResult, StoreError,
};

/// Storage backend for the feed pipeline.
///
/// Every method maps 1:1 to a D1 query.  The production implementation
/// ([`D1Store`](crate::D1Store)) wraps `worker::D1Database`; the test
/// implementation ([`MemoryStore`](crate::memory::MemoryStore)) uses
/// in-memory `HashMap`/`Vec` and supports failure injection.
#[async_trait(?Send)]
pub trait StoreBackend {
    // ---- Feeds ----

    /// Load one feed by id.
    async fn get_feed(&self, id: i64) -> Result<Option<Feed>, StoreError>;

    /// Record a fetch result (etag / last-modified) after a successful fetch.
    async fn record_fetch_result(
        &self,
        feed_id: i64,
        fetched_at: i64,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<(), StoreError>;

    // ---- Rules ----

    /// Return `rule_json` strings for every enabled rule matching `audience_tag`.
    async fn active_rule_jsons(&self, audience_tag: &str) -> Result<Vec<String>, StoreError>;

    // ---- Articles ----

    /// Insert a new article (INSERT OR IGNORE).  Returns the new row id,
    /// or `None` when the article already exists (duplicate GUID).
    async fn insert_article(&self, article: &NewArticle) -> Result<Option<i64>, StoreError>;

    /// Persist AI summarisation results.
    async fn set_ai_summary(
        &self,
        article_id: i64,
        summary: &str,
        tags_json: &str,
        vector_id: &str,
        score: f64,
    ) -> Result<(), StoreError>;

    /// Update the R2 key pointing to the article's full-text body.
    async fn set_raw_content_r2_key(&self, article_id: i64, r2_key: Option<&str>) -> Result<(), StoreError>;

    /// Delete articles older than `days` whose AI processing is complete.
    async fn expire_old_articles(&self, now: i64, days: i64) -> Result<u64, StoreError>;

    // ===== Intelligence / Entity methods =====

    /// Upsert an entity by normalized_name. Returns the entity id.
    async fn upsert_entity(&self, name: &str, normalized: &str, entity_type: &str) -> Result<i64, StoreError>;

    /// Link an article to an entity (many-to-many).
    async fn link_article_entity(
        &self,
        article_id: i64,
        entity_id: i64,
        relevance: f64,
        context: Option<&str>,
    ) -> Result<(), StoreError>;

    /// Link two entities with a directed relation.
    async fn link_entity_relation(
        &self,
        source: i64,
        target: i64,
        rtype: &str,
        confidence: f64,
    ) -> Result<(), StoreError>;

    /// List all entities, paginated, ordered by article_count DESC.
    async fn list_entities(&self, limit: u32, offset: u32) -> Result<Vec<EntitySummary>, StoreError>;

    /// Get a single entity by id with aggregate article_count.
    async fn entity_detail(&self, id: i64) -> Result<Option<EntityDetail>, StoreError>;

    /// Get related entities for a given entity through entity_relations.
    async fn entity_relations(&self, entity_id: i64, limit: u32) -> Result<Vec<RelatedEntity>, StoreError>;

    /// Get all entities linked to a specific article.
    async fn article_entities(&self, article_id: i64) -> Result<Vec<EntityRef>, StoreError>;

    /// Register an R2 artifact in the artifact_registry.
    async fn create_artifact(&self, artifact: &NewArtifact) -> Result<i64, StoreError>;

    /// List artifact_registry entries for a given entity.
    async fn list_artifacts_by_entity(&self, entity_id: i64, limit: u32) -> Result<Vec<ArtifactEntry>, StoreError>;

    // ===== Entity Intelligence methods =====

    /// List articles linked to an entity (Evidence).
    async fn entity_articles(&self, entity_id: i64, limit: u32, offset: u32) -> Result<Vec<EntityArticle>, StoreError>;

    /// Activity summary for an entity over the last N days.
    async fn entity_activity_summary(
        &self,
        entity_id: i64,
        now: i64,
        days: i64,
    ) -> Result<EntityActivitySummary, StoreError>;

    /// Generate entity-anchored signal candidates with 5-factor scoring.
    async fn entity_signal_candidates(
        &self,
        now: i64,
        days: i64,
        limit: u32,
    ) -> Result<Vec<EntitySignalCandidate>, StoreError>;

    /// Load recent articles that have Vectorize embeddings for ANN discovery.
    async fn recent_embedded_articles(
        &self,
        now: i64,
        days: i64,
        limit: u32,
    ) -> Result<Vec<ArticleEmbeddingRef>, StoreError>;

    // ===== Signal Threads (V2) =====

    async fn upsert_signal_thread(
        &self,
        signal_key: &str,
        anchor_entity_id: Option<i64>,
        title: &str,
        status: &str,
        discovery_method: &DiscoveryMethod,
        discovery_score: Option<f64>,
    ) -> Result<SignalUpsertResult, StoreError>;

    async fn update_signal_lifecycle(&self, now: i64) -> Result<(), StoreError>;

    async fn get_active_signal_threads(&self, limit: u32) -> Result<Vec<SignalBriefInput>, StoreError>;

    /// List signal threads with dynamic filtering.
    async fn list_signal_threads(&self, filter: &SignalThreadFilter) -> Result<Vec<SignalBriefInput>, StoreError>;

    async fn load_signal_detail(&self, thread_id: i64) -> Result<Option<SignalDetail>, StoreError>;

    // ===== Signal Engine V2 methods =====

    /// Generate entity-anchored signal candidates with quality filters.
    /// - `min_entity_articles`: minimum articles linked to entity (anti-noise).
    /// - `min_sources`: minimum distinct feed sources (requires corroboration).
    async fn entity_signal_candidates_filtered(
        &self,
        now: i64,
        days: i64,
        limit: u32,
        min_entity_articles: u32,
        min_sources: u32,
    ) -> Result<Vec<EntitySignalCandidate>, StoreError>;

    /// Append a signal instance with enriched snapshot (avg_score, entity_id).
    #[allow(clippy::too_many_arguments)]
    async fn append_signal_instance_v2(
        &self,
        thread_id: i64,
        score: f64,
        impact: &str,
        trend: &str,
        article_count: i64,
        source_count: i64,
        avg_score: f64,
        entity_id: i64,
    ) -> Result<i64, StoreError>;

    /// Insert a signal timeline event for a thread.
    async fn insert_signal_event(
        &self,
        thread_id: i64,
        event_type: &str,
        payload: Option<&str>,
    ) -> Result<(), StoreError>;

    /// Load signal timeline events for a thread.
    async fn load_signal_events(&self, thread_id: i64, limit: u32) -> Result<Vec<SignalEvent>, StoreError>;

    /// Load related entities for a signal thread via entity_relations.
    async fn load_thread_related_entities(
        &self,
        thread_id: i64,
        limit: u32,
    ) -> Result<Vec<RelatedEntityRef>, StoreError>;

    // ===== Decision Loop =====

    /// Create a new decision record.
    async fn create_decision(&self, d: &NewDecision) -> Result<i64, StoreError>;

    /// Get a single decision by id.
    async fn get_decision(&self, id: i64) -> Result<Option<Decision>, StoreError>;

    /// List decisions, optionally filtered by status.
    async fn list_decisions(&self, status: Option<&str>, limit: u32) -> Result<Vec<Decision>, StoreError>;

    /// Update decision status.
    async fn update_decision_status(&self, id: i64, status: &str) -> Result<(), StoreError>;

    /// List decisions for a specific signal thread.
    async fn decisions_by_signal(&self, signal_thread_id: i64) -> Result<Vec<Decision>, StoreError>;

    // ===== Outcome Events =====

    /// Record a factual outcome observation.
    async fn create_outcome(&self, e: &NewOutcomeEvent) -> Result<i64, StoreError>;

    /// List outcome observations for a decision.
    async fn get_decision_outcomes(&self, decision_id: i64) -> Result<Vec<OutcomeEvent>, StoreError>;

    // ===== Decision Evaluation =====

    /// Record a judgment about whether a decision's hypothesis was correct.
    async fn create_evaluation(&self, e: &NewDecisionEvaluation) -> Result<i64, StoreError>;

    /// List all evaluations for a decision, newest first.
    async fn get_decision_evaluations(&self, decision_id: i64) -> Result<Vec<DecisionEvaluation>, StoreError>;

    /// Get the latest evaluation for a decision.
    async fn get_latest_evaluation(&self, decision_id: i64) -> Result<Option<DecisionEvaluation>, StoreError>;
}
