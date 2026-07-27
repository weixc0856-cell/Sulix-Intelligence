//! Shared kernel for the Sulix Intelligence platform.
//!
//! **Zero infrastructure dependencies.**  Pure domain types, value objects,
//! event definitions, and error types shared across all bounded contexts.
//!
//! # Crate dependencies
//! - `serde` / `serde_json` — serialisation (required by events)
//! - `thiserror` — error derive
//! - `fastrand` — event ID generation (lightweight, no system deps)

pub mod artifact_registry;
pub mod error;
pub mod events;
pub mod ids;
pub mod time;

pub use error::DomainError;
pub use events::*;
pub use ids::*;
pub use time::Timestamp;
