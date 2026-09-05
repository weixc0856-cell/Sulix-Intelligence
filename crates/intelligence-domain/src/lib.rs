//! Intelligence Domain — the core cognitive context.
//!
//! This crate consolidates Observation, Claim, Signal, and Evidence into
//! a single bounded context with deep module entry points.
//!
//! ## Public surface
//!
//! - [`IntelligenceEngine`] — single entry point (`observe`, `analyze`, `detect_signals`)
//! - Repository traits: `ObservationRepository`, `ClaimRepository`, `SignalRepository`
//! - Domain types: `Observation`, `Claim`, `SignalThread`, `EvidenceRef`
//!
//! ## Relationship to old crates
//!
//! The old `claim-engine` and `signal-engine` crates now re-export from here.
//! They will be deprecated in a future sprint.

mod claim;
pub mod confidence;
mod engine;
mod error;
mod observation;
mod repositories;
mod signal;

// ── Public API ───────────────────────────────────────────

pub use claim::{Claim, ClaimType, EvidenceRef, EvidenceRelation, NewClaim};
pub use confidence::calculator::calculate;
pub use confidence::factors::{ConfidenceFactorExplanation, ConfidenceFactors, ConfidenceResult};
pub use confidence::policy::ConfidencePolicy;
pub use engine::IntelligenceEngine;
pub use error::IntelligenceError;
pub use observation::{NewObservation, Observation};
pub use repositories::{ClaimRepository, ObservationRepository, SignalRepository};
pub use signal::{NewSignalThread, SignalStatus, SignalThread};
