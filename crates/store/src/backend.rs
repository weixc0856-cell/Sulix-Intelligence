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

use domain::traits::*;

/// Storage backend for the Sulix Intelligence platform.
///
/// Composes all domain-repository and query-service traits so that existing
/// `T: StoreBackend` generic code continues to compile as we migrate toward
/// smaller, context-specific boundaries.
///
/// Empty composite body: every capability now lives on a smaller [`traits`]
/// subtrait. The legacy decision-write vertical lives on
/// [`DecisionWriteStore`], which this trait also composes so that
/// `T: StoreBackend` callers (worker-entry's production DecisionService) keep
/// the four write methods unchanged (decoupling plan §5, GATED).
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
    + DecisionWriteStore
{
}
