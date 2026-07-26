//! `StoreBackend` trait — supertrait composing all domain-repository and
//! query-service traits.
//!
//! New code should prefer the smaller traits from [`traits`] so the
//! dependency graph stays lean.  Legacy code that uses `T: StoreBackend`
//! continues to compile without changes because `StoreBackend` is a
//! supertrait of every smaller trait.
//!
//! MVP scope: only the methods used by the feed processing pipeline.

use async_trait::async_trait;

use crate::{
    traits::*, ArtifactEntry, ArtifactRecord, Claim, ClaimEvidence, ConfidenceEvent, ContextSnapshot, Decision,
    DecisionEvaluation, DiscoveryMethod, EntitySignalCandidate, EventIndexEntry, Memory, NewArticle, NewArtifact,
    NewArtifactRecord, NewClaim, NewConfidenceEvent, NewContextSnapshot, NewDecision, NewDecisionEvaluation, NewMemory,
    NewObservation, NewOutbox, NewOutcomeEvent, NewReflection, Observation, OutboxEntry, OutcomeEvent, Reflection,
    RelatedEntityRef, SignalDetail, SignalEvent, SignalUpsertResult, StoreError, UpdateReflection,
};

/// Storage backend for the Sulix Intelligence platform.
///
/// Composes all domain-repository and query-service traits so that existing
/// `T: StoreBackend` generic code continues to compile as we migrate toward
/// smaller, context-specific boundaries.
///
/// **Phase 1** — the following method groups remain on `StoreBackend`:
/// - Article lifecycle (set_ai_summary, set_raw_content_r2_key, expire)
/// - Rule management (active_rule_jsons)
/// - Entity signal candidates (bridge to Intelligence context)
/// - Signal instance & event append (pre-cursors to formal Event Sourcing)
/// - Decision status + outcome + evaluation CRUD
/// - Outbox / Event Index (infrastructure, will move to shared/events)
/// - Reflection, Memory, Context, Artifact CRUD
#[async_trait(?Send)]
pub trait StoreBackend:
    FeedRepository
    + FeedQueryService
    + ArticleRepository
    + ArticleQueryService
    + EntityRepository
    + EntityQueryService
    + SignalRepository
    + SignalQueryService
    + DecisionRepository
    + DecisionQueryService
    + OutcomeRepository
    + OutcomeQueryService
    + EvaluationRepository
    + EvaluationQueryService
    + BatchSignalQueryService
    + ClaimRepository
    + ObservationRepository
    + ConfidenceRepository
    + ClaimQueryService
{
    // ---- Rules ----

    /// Return `rule_json` strings for every enabled rule matching `audience_tag`.
    async fn active_rule_jsons(&self, audience_tag: &str) -> Result<Vec<String>, StoreError>;

    // ---- Article lifecycle (analysis / content) ----

    /// Insert a new article (called by ingestion; maps to ArticleRepository::save_article).
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

    // ---- Feed lifecycle ----

    /// Record a fetch result (etag / last-modified) after a successful fetch.
    async fn record_fetch_result(
        &self,
        feed_id: i64,
        fetched_at: i64,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<(), StoreError>;

    // ---- Entity lifecycle (compat aliases for ingestion) ----

    /// Upsert an entity (called by ingestion; maps to EntityRepository::save_entity).
    async fn upsert_entity(&self, name: &str, normalized: &str, entity_type: &str) -> Result<i64, StoreError>;

    /// Link article to entity (called by ingestion; maps to EntityRepository::link_article).
    async fn link_article_entity(
        &self,
        article_id: i64,
        entity_id: i64,
        relevance: f64,
        context: Option<&str>,
    ) -> Result<(), StoreError>;

    /// Link two entities (called by ingestion; maps to EntityRepository::link_relation).
    async fn link_entity_relation(
        &self,
        source: i64,
        target: i64,
        rtype: &str,
        confidence: f64,
    ) -> Result<(), StoreError>;

    // ---- Entity signal candidates (bridge to Intelligence context) ----

    /// Generate entity-anchored signal candidates with 5-factor scoring.
    async fn entity_signal_candidates(
        &self,
        now: i64,
        days: i64,
        limit: u32,
    ) -> Result<Vec<EntitySignalCandidate>, StoreError>;

    /// Generate entity-anchored signal candidates with quality filters.
    async fn entity_signal_candidates_filtered(
        &self,
        now: i64,
        days: i64,
        limit: u32,
        min_entity_articles: u32,
        min_sources: u32,
    ) -> Result<Vec<EntitySignalCandidate>, StoreError>;

    // ==== Signal instance & event management (pre-Event-Sourcing) ====

    /// Upsert a signal thread (called by signal-engine; maps to SignalRepository::save_signal).
    async fn upsert_signal_thread(
        &self,
        signal_key: &str,
        anchor_entity_id: Option<i64>,
        title: &str,
        status: &str,
        discovery_method: &DiscoveryMethod,
        discovery_score: Option<f64>,
    ) -> Result<SignalUpsertResult, StoreError>;

    /// Update signal lifecycle (active → decaying → resolved → archived).
    async fn update_signal_lifecycle(&self, now: i64) -> Result<(), StoreError>;

    /// Load full signal detail (thread info + timeline + evidence + entities).
    async fn load_signal_detail(&self, thread_id: i64) -> Result<Option<SignalDetail>, StoreError>;

    /// Get the latest instance's (score, trend) for dedup.
    async fn get_latest_instance_fingerprint(&self, thread_id: i64) -> Result<Option<(f64, String)>, StoreError>;

    /// Append a daily signal instance snapshot.
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

    /// Insert a signal timeline event.
    async fn insert_signal_event(
        &self,
        thread_id: i64,
        event_type: &str,
        payload: Option<&str>,
    ) -> Result<(), StoreError>;

    /// Load signal timeline events.
    async fn load_signal_events(&self, thread_id: i64, limit: u32) -> Result<Vec<SignalEvent>, StoreError>;

    /// Load related entities for a signal thread.
    async fn load_thread_related_entities(
        &self,
        thread_id: i64,
        limit: u32,
    ) -> Result<Vec<RelatedEntityRef>, StoreError>;

    // ==== Decision lifecycle (pre-Event-Sourcing) ====

    /// Create a new decision (called by api/services/decision.rs; maps to DecisionRepository::save_decision).
    async fn create_decision(&self, d: &NewDecision) -> Result<i64, StoreError>;

    /// Get a decision by id (called by reflection-engine; maps to DecisionRepository::find_decision).
    async fn get_decision(&self, id: i64) -> Result<Option<Decision>, StoreError>;

    /// Update decision status.
    async fn update_decision_status(&self, id: i64, status: &str) -> Result<(), StoreError>;

    // ---- Outcome Events ----

    /// Record a factual outcome observation.
    async fn create_outcome(&self, e: &NewOutcomeEvent) -> Result<i64, StoreError>;

    /// List outcome observations for a decision.
    async fn get_decision_outcomes(&self, decision_id: i64) -> Result<Vec<OutcomeEvent>, StoreError>;

    // ---- Decision Evaluation ----

    /// Record a judgment about whether a decision's hypothesis was correct.
    async fn create_evaluation(&self, e: &NewDecisionEvaluation) -> Result<i64, StoreError>;

    /// List all evaluations for a decision, newest first.
    async fn get_decision_evaluations(&self, decision_id: i64) -> Result<Vec<DecisionEvaluation>, StoreError>;

    /// Get the latest evaluation for a decision.
    async fn get_latest_evaluation(&self, decision_id: i64) -> Result<Option<DecisionEvaluation>, StoreError>;

    // ==== Object Outbox (infrastructure) ====

    /// Enqueue a new outbox entry for deferred R2 archive write.
    async fn insert_outbox(&self, entry: &NewOutbox) -> Result<i64, StoreError>;

    /// Drain up to `limit` pending outbox entries, oldest first.
    async fn drain_outbox(&self, limit: u32) -> Result<Vec<OutboxEntry>, StoreError>;

    /// Mark an outbox entry as successfully archived.
    async fn mark_outbox_archived(&self, id: i64) -> Result<(), StoreError>;

    /// Mark an outbox entry as failed (retries exhausted).
    async fn mark_outbox_failed(&self, id: i64) -> Result<(), StoreError>;

    // ==== Event Archive Index (infrastructure) ====

    /// Insert a row into the event_archive_index table.
    async fn insert_event_index(
        &self,
        event_id: &str,
        aggregate_type: &str,
        aggregate_id: &str,
        event_type: &str,
        object_key: &str,
        occurred_at: i64,
    ) -> Result<(), StoreError>;

    /// Find event index entries for an aggregate, newest first.
    async fn find_event_keys(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
        limit: u32,
    ) -> Result<Vec<EventIndexEntry>, StoreError>;

    // ==== Artifact Registry ====

    /// Register an R2 artifact in the artifact_registry.
    async fn create_artifact(&self, artifact: &NewArtifact) -> Result<i64, StoreError>;

    /// List artifact_registry entries for a given entity.
    async fn list_artifacts_by_entity(&self, entity_id: i64, limit: u32) -> Result<Vec<ArtifactEntry>, StoreError>;

    /// Register a new artifact in the memory_artifacts index.
    async fn put_artifact(&self, artifact: &NewArtifactRecord) -> Result<i64, StoreError>;

    /// Retrieve an artifact record by type + date.
    async fn get_artifact(&self, artifact_type: &str, date: &str) -> Result<Option<ArtifactRecord>, StoreError>;

    /// List artifacts of a given type, newest first.
    async fn list_artifacts(&self, artifact_type: &str, limit: u32) -> Result<Vec<ArtifactRecord>, StoreError>;

    // ===== Reflection Engine (Sprint 5.4) =====

    async fn create_reflection(&self, req: &NewReflection) -> Result<i64, StoreError>;
    async fn update_reflection(&self, req: &UpdateReflection) -> Result<(), StoreError>;
    async fn get_reflection_by_decision(&self, decision_id: i64) -> Result<Option<Reflection>, StoreError>;
    async fn decisions_eligible_for_reflection(&self, now: i64, limit: u32) -> Result<Vec<i64>, StoreError>;
    async fn failed_reflections_for_retry(&self, limit: u32) -> Result<Vec<Reflection>, StoreError>;
    async fn stale_generating_reflections(&self, now: i64) -> Result<Vec<Reflection>, StoreError>;

    // ===== Claim (Sprint 5.3) =====

    async fn create_claim(&self, c: &NewClaim) -> Result<i64, StoreError>;
    async fn get_claim(&self, id: i64) -> Result<Option<Claim>, StoreError>;
    async fn list_claims(&self, status: Option<&str>, limit: u32) -> Result<Vec<Claim>, StoreError>;
    async fn get_claim_evidence(&self, claim_id: i64) -> Result<Vec<ClaimEvidence>, StoreError>;
    // ===== Observation (Sprint 5.4) =====

    async fn create_observation(&self, o: &NewObservation) -> Result<i64, StoreError>;
    async fn get_observation(&self, id: i64) -> Result<Option<Observation>, StoreError>;
    async fn find_observation_by_hash(&self, hash: &str) -> Result<Option<Observation>, StoreError>;

    // ===== Confidence Event (Sprint 5.4B) =====

    /// Append a confidence-change event. Returns the new event id.
    async fn append_confidence(&self, event: &NewConfidenceEvent) -> Result<i64, StoreError>;

    /// List confidence history for one entity, ascending by created_at.
    async fn list_confidence_history(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Vec<ConfidenceEvent>, StoreError>;

    // ===== Memory Engine (Sprint 5.5) =====

    async fn create_memory(&self, entry: &NewMemory) -> Result<i64, StoreError>;
    async fn get_memory(&self, id: i64) -> Result<Option<Memory>, StoreError>;
    async fn list_memories(
        &self,
        memory_type: Option<&str>,
        status: Option<&str>,
        limit: u32,
    ) -> Result<Vec<Memory>, StoreError>;
    async fn touch_memory(&self, id: i64, now: i64) -> Result<(), StoreError>;
    async fn count_candidate_memories(&self) -> Result<i64, StoreError>;

    // ===== Context Engine (Sprint 5.6) =====

    async fn save_context_snapshot(&self, snap: &NewContextSnapshot) -> Result<(), StoreError>;
    async fn get_context_snapshot(&self, id: &str) -> Result<Option<ContextSnapshot>, StoreError>;
}
