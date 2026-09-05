//! Trait definitions for the DDD bounded-context boundaries.
//!
//! - [`repo`] — Aggregate persistence (save / find).  ~2-3 methods each.
//! - [`query`] — Read-model queries (list, radar, detail, stats).  ~5-15 methods each.
//!
//! The legacy [`StoreBackend`](crate::StoreBackend) supertrait composes all of
//! the above, so existing `T: StoreBackend` generic code continues to compile
//! without changes.

pub mod article_analysis_store;
pub mod artifact_store;
pub mod context_snapshot_store;
pub mod event_index_store;
pub mod memory_persistence;
pub mod outbox_store;
pub mod query;
pub mod reflection_persistence;
pub mod repo;
pub mod signal_store;

pub use article_analysis_store::ArticleAnalysisStore;
pub use artifact_store::ArtifactStore;
pub use context_snapshot_store::ContextSnapshotStore;
pub use event_index_store::EventIndexStore;
pub use memory_persistence::MemoryPersistence;
pub use outbox_store::OutboxStore;
pub use query::*;
pub use reflection_persistence::ReflectionPersistence;
pub use repo::*;
pub use signal_store::SignalStore;
