//! Trait definitions for the DDD bounded-context boundaries.
//!
//! - [`repo`] — Aggregate persistence (save / find).  ~2-3 methods each.
//! - [`query`] — Read-model queries (list, radar, detail, stats).  ~5-15 methods each.
//!
//! Callers depend on exactly the narrow trait(s) they call; there is no legacy
//! composite supertrait anymore.

pub mod article_analysis_store;
pub mod artifact_store;
pub mod briefing_store;
pub mod context_snapshot_store;
pub mod decision_id_source;
pub mod decision_record_store;
pub mod decision_upsert;
pub mod event_index_store;
pub mod memory_persistence;
pub mod metrics_store;
pub mod outbox_store;
pub mod query;
pub mod reflection_persistence;
pub mod repo;
pub mod rule_store;
pub mod signal_store;
pub mod takedown_store;

pub use article_analysis_store::ArticleAnalysisStore;
pub use artifact_store::ArtifactStore;
pub use briefing_store::BriefingStore;
pub use context_snapshot_store::ContextSnapshotStore;
pub use decision_id_source::DecisionIdSource;
pub use decision_record_store::DecisionRecordStore;
pub use decision_upsert::DecisionUpsertStore;
pub use event_index_store::EventIndexStore;
pub use memory_persistence::MemoryPersistence;
pub use metrics_store::MetricsStore;
pub use outbox_store::OutboxStore;
pub use query::*;
pub use reflection_persistence::ReflectionPersistence;
pub use repo::*;
pub use rule_store::RuleStore;
pub use signal_store::SignalStore;
pub use takedown_store::TakedownStore;
