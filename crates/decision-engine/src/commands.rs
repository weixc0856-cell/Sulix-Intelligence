//! Command structs for the Decision aggregate.
//!
//! These are pure data — they carry intent from the application layer
//! into the domain. Every public method on `DecisionAggregate` accepts
//! a command struct, not a grab-bag of parameters.
//!
//! Commands are **infrastructure-free**: no HTTP, no D1, no Queue types.
//! They can be constructed by API handlers, Queue consumers, or Agent
//! runtimes with equal ease.

use shared_kernel::ids::DecisionId;

use crate::outcome::{ExpectedOutcome, ObservedOutcome};
use crate::status::DecisionStatus;

/// Propose a new decision.
#[derive(Debug, Clone)]
pub struct ProposeDecision {
    /// System-assigned primary key.
    pub id: i64,
    pub title: String,
    pub hypothesis: Option<String>,
    pub confidence: f64,
    pub rationale: Option<String>,
    pub decision_type: String,
    pub priority: String,
    pub signal_thread_id: Option<i64>,
    pub actor_id: Option<i64>,
    pub expected_outcomes: Vec<ExpectedOutcome>,
}

/// Reconstruct an aggregate from persisted state.
///
/// Unlike the other commands, this is a **hydration input**, not an intent:
/// it trusts the data came from a valid prior state, performs no business
/// validation, and emits no domain events. Repository implementations use
/// it to hydrate aggregates from D1 rows.
#[derive(Debug, Clone)]
pub struct ReconstructDecision {
    pub id: DecisionId,
    pub title: String,
    pub hypothesis: Option<String>,
    pub confidence: f64,
    pub status: DecisionStatus,
    pub rationale: Option<String>,
    pub decision_type: String,
    pub priority: String,
    pub signal_thread_id: Option<i64>,
    pub actor_id: Option<i64>,
    pub expected_outcomes: Vec<ExpectedOutcome>,
    pub observed_outcomes: Vec<ObservedOutcome>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Approve a proposed decision.
#[derive(Debug, Clone)]
pub struct ApproveDecision {
    pub decision_id: String,
    pub approved_by: String,
}

/// Start executing an approved decision.
#[derive(Debug, Clone)]
pub struct ExecuteDecision {
    pub decision_id: String,
}

/// Attach a real-world outcome observation.
#[derive(Debug, Clone)]
pub struct RecordOutcome {
    pub decision_id: String,
    pub outcome: ObservedOutcome,
}

/// Invalidate a decision (mark as no longer relevant).
#[derive(Debug, Clone)]
pub struct InvalidateDecision {
    pub decision_id: String,
    pub reason: String,
}
