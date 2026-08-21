//! Reasoning Framework Engine — structured mental models for judgment.
//!
//! Sprint 6.4: Adds a reasoning layer between Claim extraction and Decision
//! formation. Each framework is a calibrated mental model with trigger rules,
//! reasoning templates, and historical accuracy tracking.
//!
//! ## Architecture
//!
//! ```text
//! Claim arrives → ReasoningSelector selects frameworks by context
//!     ↓
//! Selected frameworks injected into LLM prompt
//!     ↓
//! LLM returns claim + frameworks_applied
//!     ↓
//! Confidence delta recorded (before/after)
//!     ↓
//! Outcome recorded → CalibrationEngine updates framework scores
//! ```
//!
//! ## Public surface
//!
//! - [`ReasoningFramework`] — framework entity with trigger rules and calibration
//! - [`FrameworkCategory`] — 6 categories (Mathematics, Finance, Behavior, etc.)
//! - [`FrameworkRepository`] — domain-owned persistence contract
//! - [`ReasoningSelector`] — rule-based framework matching
//! - [`CalibrationEngine`] — outcome-driven accuracy tracking

mod calibration;
mod error;
mod framework;
mod repository;
mod seed;
mod selector;

pub use calibration::CalibrationEngine;
pub use error::FrameworkError;
pub use framework::{
    ClaimFrameworkRef, FrameworkCategory, FrameworkImpact, NewFramework, ReasoningFramework, TriggerRule,
};
pub use repository::FrameworkRepository;
pub use seed::{initial_frameworks, seed_count};
pub use selector::ReasoningSelector;
