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

pub mod artifact_registry;
pub mod storage_policy;
