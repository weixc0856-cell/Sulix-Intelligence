//! Application layer — UseCase orchestration services.
//!
//! Every UseCase in this crate:
//! - Is **generic over its dependencies** (typically `StoreBackend` subtraits)
//! - Contains **zero HTTP / Worker code**
//! - Is **unit-testable** with `MemoryStore`
//! - Returns domain types, not HTTP responses
//!
//! API controllers in the `api` crate parse HTTP requests, call these
//! services, and convert results to JSON responses.

pub mod graph;
pub mod provenance;
pub mod radar;
pub mod semantic_search;
pub mod services;

pub use graph::{ExpandRequest, ExpandResponse, GraphProjectionService};
pub use provenance::{get_lineage, ProvenanceChain, ProvenanceNode};
pub use radar::RadarProjectionService;
pub use semantic_search::SemanticSearchService;
pub use services::articles::ArticleService;
pub use services::briefing::BriefingService;
pub use services::claims::ClaimService;
pub use services::compliance::ComplianceService;
pub use services::confidence::ConfidenceService;
pub use services::decision::DecisionService;
pub use services::decision_read::DecisionReadService;
pub use services::entities::EntityService;
pub use services::feeds::FeedService;
pub use services::observations::ObservationService;
pub use services::rules::{RuleError, RuleService};
pub use services::sources::SourceService;
pub use services::strategies::StrategyPreviewService;
pub use services::system::SystemService;
pub use services::trust::TrustService;
