//! Infrastructure Layer — concrete implementations of domain contracts.
//!
//! Every public type in this crate is an **implementation** of a trait defined
//! in `shared-kernel` or a domain crate (decision-engine, intelligence-domain,
//! etc.). Infrastructure depends on domain; domain never depends on
//! infrastructure.
//!
//! ## Modules
//!
//! - [`artifact_registry`] — ArtifactRegistry impl (InMemory for tests, D1 for production)
//! - [`storage_policy`] — Source governance ↔ artifact retention policy
//! - [`signal_repository`] — signal-engine persistence + discovery adapters

pub mod article_persistence;
pub mod artifact_registry;
pub mod context_repository;
pub mod decision_repository;
pub mod event_log;
pub mod memory_repository;
pub mod provenance;
pub mod reflection_repository;
pub mod semantic_query;
pub mod signal_event_log;
pub mod signal_repository;
pub mod storage_policy;
