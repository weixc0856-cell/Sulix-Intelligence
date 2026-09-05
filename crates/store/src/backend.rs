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
    traits::*, Decision, DecisionEvaluation, NewDecision, NewDecisionEvaluation, NewOutcomeEvent, OutcomeEvent,
    StoreError,
};

/// Storage backend for the Sulix Intelligence platform.
///
/// Composes all domain-repository and query-service traits so that existing
/// `T: StoreBackend` generic code continues to compile as we migrate toward
/// smaller, context-specific boundaries.
///
/// Only the decision vertical remains on the body (shrunken by P4 batches 0–9);
/// every other capability now lives on a smaller [`traits`] subtrait. The
/// decision vertical is ported in the decoupling plan §5 (GATED).
#[async_trait(?Send)]
pub trait StoreBackend:
    FeedRepository
    + FeedQueryService
    + ArticleRepository
    + ArticleAnalysisStore
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
    + ArtifactStore
    + RuleStore
    + SignalStore
{
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
}
