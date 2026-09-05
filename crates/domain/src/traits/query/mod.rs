//! Read-model query traits — **read operations only**.
//!
//! Each QueryService trait matches a bounded context and mirrors the read
//! methods that were historically part of `StoreBackend`.  Write-side
//! persistence for the corresponding aggregate lives in
//! [`super::repo`].

pub mod article_query;
pub mod claim_query;
pub mod decision_query;
pub mod entity_query;
pub mod evaluation_query;
pub mod feed_query;
pub mod observation_query;
pub mod outcome_query;
pub mod signal_query;

pub use article_query::ArticleQueryService;
pub use claim_query::ClaimQueryService;
pub use decision_query::DecisionQueryService;
pub use entity_query::EntityQueryService;
pub use evaluation_query::EvaluationQueryService;
pub use feed_query::FeedQueryService;
pub use observation_query::ObservationQueryService;
pub use outcome_query::OutcomeQueryService;
pub use signal_query::{BatchSignalQueryService, SignalQueryService};
pub mod source_query;
pub use source_query::SourceQueryService;
