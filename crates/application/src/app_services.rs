//! Composition bundle wiring every application service to one store.
//!
//! `AppServices<S>` is generic over the narrow store subtraits each service
//! needs; the D1-backed production alias `ProductionAppServices =
//! AppServices<D1Store>` lives in the `composition` crate (the only place
//! `application` and `store` are co-visible), so `api`/`worker-entry` can name
//! the concrete bundle without `api` depending on `store`.

use crate::graph::GraphProjectionService;
use crate::services::articles::ArticleService;
use crate::services::claims::ClaimService;
use crate::services::compliance::ComplianceService;
use crate::services::confidence::ConfidenceService;
use crate::services::decision_read::DecisionReadService;
use crate::services::entities::EntityService;
use crate::services::feeds::FeedService;
use crate::services::observations::ObservationService;
use crate::services::rules::RuleService;
use crate::services::sources::SourceService;
use crate::services::strategies::StrategyPreviewService;
use crate::services::system::SystemService;
use crate::services::trust::TrustService;

/// Generic composition bundle over any store that satisfies the union of the
/// service constructor bounds (the full `MemoryStore`/`D1Store` surface).
///
/// `new` only builds the service graph; individual methods keep their own
/// narrow `where` clauses and are monomorphized against the concrete `S`.
pub struct AppServices<S> {
    /// Raw store handle for delivery-layer infrastructure routes
    /// (`worker-entry` only). `api` handlers never read this field.
    pub store: S,
    pub article: ArticleService<S>,
    pub source: SourceService<S>,
    pub entity: EntityService<S>,
    pub feed: FeedService<S>,
    pub rule: RuleService<S>,
    pub compliance: ComplianceService<S>,
    pub trust: TrustService<S>,
    pub system: SystemService<S>,
    pub confidence: ConfidenceService<S>,
    pub claim: ClaimService<S>,
    pub observation: ObservationService<S>,
    pub decision_read: DecisionReadService<S>,
    pub strategy_preview: StrategyPreviewService<S>,
    pub graph: GraphProjectionService<S>,
}

impl<S> AppServices<S>
where
    S: Clone
        + store::ArticleQueryService
        + store::ClaimQueryService
        + store::ClaimRepository
        + store::ConfidenceRepository
        + store::DecisionQueryService
        + store::DecisionRecordStore
        + store::DecisionRepository
        + store::EntityQueryService
        + store::FeedQueryService
        + store::FeedRepository
        + store::MetricsStore
        + store::ObservationQueryService
        + store::ObservationRepository
        + store::ReflectionPersistence
        + store::RuleStore
        + store::SignalQueryService
        + store::SignalStore
        + store::SourceQueryService
        + store::SourceRepository
        + store::TakedownStore,
{
    /// Build every service over one `store` value. Each service holds its own
    /// clone of the handle; the original is retained as `self.store`.
    pub fn new(store: S) -> Self {
        Self {
            article: ArticleService::new(store.clone()),
            source: SourceService::new(store.clone()),
            entity: EntityService::new(store.clone()),
            feed: FeedService::new(store.clone()),
            rule: RuleService::new(store.clone()),
            compliance: ComplianceService::new(store.clone()),
            trust: TrustService::new(store.clone()),
            system: SystemService::new(store.clone()),
            confidence: ConfidenceService::new(store.clone()),
            claim: ClaimService::new(store.clone()),
            observation: ObservationService::new(store.clone()),
            decision_read: DecisionReadService::new(store.clone()),
            strategy_preview: StrategyPreviewService::new(store.clone()),
            graph: GraphProjectionService::new(store.clone()),
            store,
        }
    }
}
