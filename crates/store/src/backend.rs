//! `StoreBackend` trait — supertrait composing all domain-repository and
//! query-service traits.
//!
//! ╔══════════════════════════════════════════════════════════════════╗
//! ║  DEPRECATED — Sprint 6.2 Phase 0                               ║
//! ║                                                                  ║
//! ║  No new methods may be added to this trait.                      ║
//! ║                                                                  ║
//! ║  New domain capabilities MUST define their own repository        ║
//! ║  interface in the owning domain crate (e.g. decision-domain,     ║
//! ║  claim-domain). See Sprint 6.2 plan.                             ║
//! ║                                                                  ║
//! ║  Existing methods remain for backward compat. They will be       ║
//! ║  removed in Sprint 6.2D when StoreBackend is deleted.             ║
//! ╚══════════════════════════════════════════════════════════════════╝
//!
//! New code should prefer the smaller traits from [`traits`] so the
//! dependency graph stays lean.  Legacy code that uses `T: StoreBackend`
//! continues to compile without changes because `StoreBackend` is a
//! supertrait of every smaller trait.

use async_trait::async_trait;

use crate::{
    traits::*, ArtifactEntry, ArtifactRecord, Claim, ClaimEvidence, ConfidenceEvent, Decision, DecisionEvaluation,
    NewArticle, NewArtifact, NewArtifactRecord, NewClaim, NewConfidenceEvent, NewDecision, NewDecisionEvaluation,
    NewObservation, NewOutcomeEvent, NewSource, Observation, OutcomeEvent, Source, StoreError,
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
    + SourceRepository
    + SourceQueryService
    + ObservationQueryService
    + ClaimQueryService
    + OutboxStore
    + EventIndexStore
    + MemoryPersistence
    + ContextSnapshotStore
    + ReflectionPersistence
    + SignalStore
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

    // ===== Claim (Sprint 5.3) =====

    async fn create_claim(&self, c: &NewClaim) -> Result<i64, StoreError>;
    async fn get_claim(&self, id: i64) -> Result<Option<Claim>, StoreError>;
    async fn list_claims(&self, status: Option<&str>, limit: u32) -> Result<Vec<Claim>, StoreError>;
    async fn get_claim_evidence(&self, claim_id: i64) -> Result<Vec<ClaimEvidence>, StoreError>;
    // ===== Observation (Sprint 5.4) =====

    async fn create_observation(&self, o: &NewObservation) -> Result<i64, StoreError>;
    async fn get_observation(&self, id: i64) -> Result<Option<Observation>, StoreError>;
    async fn find_observation_by_hash(&self, hash: &str) -> Result<Option<Observation>, StoreError>;
    async fn list_observations(
        &self,
        source_type: Option<&str>,
        source_id: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Observation>, StoreError>;

    // ===== Confidence Event (Sprint 5.4B) =====

    /// Append a confidence-change event. Returns the new event id.
    async fn append_confidence(&self, event: &NewConfidenceEvent) -> Result<i64, StoreError>;

    /// List confidence history for one entity, ascending by created_at.
    async fn list_confidence_history(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Vec<ConfidenceEvent>, StoreError>;

    // ===== Source Registry (Sprint 5.6) =====

    /// Create or update a source entry.
    async fn save_source(&self, source: &NewSource) -> Result<i64, StoreError>;

    /// Get a source by its primary key.
    async fn find_source(&self, id: i64) -> Result<Option<Source>, StoreError>;

    /// Get a source by feed_id.
    async fn find_source_by_feed(&self, feed_id: i64) -> Result<Option<Source>, StoreError>;

    /// List sources with optional filters.
    async fn list_sources(
        &self,
        tier: Option<&str>,
        policy: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Source>, StoreError>;
}
