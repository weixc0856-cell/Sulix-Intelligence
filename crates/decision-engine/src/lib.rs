//! Decision Engine — Domain-driven Decision Context.
//!
//! This is a **deep module**: external code interacts only with
//! `DecisionAggregate`, `DecisionRepository`, and the command structs.
//! All internal complexity (state machine, outcome management, event
//! production) is hidden behind `mod` declarations.
//!
//! ## Public surface (new — Sprint 6.2A)
//!
//! - [`DecisionAggregate`] — Root aggregate with behavioral methods.
//! - [`DecisionRepository`] — Domain-owned persistence contract.
//! - Commands: `ProposeDecision`, `ApproveDecision`, `ExecuteDecision`,
//!   `RecordOutcome`, `InvalidateDecision`.
//! - Supporting types: `DecisionStatus`, `DecisionDomainEvent`,
//!   `ExpectedOutcome`, `ObservedOutcome`, `OutcomeVerdict`, `DecisionError`.
//!
//! ## Public surface (legacy — Sprint 6.0, maintained for backward compat)
//!
//! - `generate_memo` — 12-section Decision Memo generator.
//! - `build_proposal` — Signal-to-Decision proposal builder.
//! - Domain types: `DecisionMemo`, `DecisionProposal`, `MemoSection`,
//!   `DecisionStatus` (legacy string-based), `OutcomeStatus`.

mod aggregate;
mod commands;
mod error;
mod events;
mod outcome;
mod repository;
mod status;

// Legacy modules (Sprint 6.0) — kept for backward compat during 6.2 transition.
pub mod domain;
pub mod memo;
pub mod proposal;

// ── Public API (new) ───────────────────────────────────────────

pub use aggregate::DecisionAggregate;
pub use commands::{ApproveDecision, ExecuteDecision, InvalidateDecision, ProposeDecision, RecordOutcome};
pub use error::DecisionError;
pub use events::DecisionDomainEvent;
pub use outcome::{ExpectedOutcome, ObservedOutcome, OutcomeVerdict};
pub use repository::DecisionRepository;
pub use status::DecisionStatus;

// ── Public API (legacy backward compat) ────────────────────────

pub use domain::{DecisionMemo, DecisionProposal, MemoSection, OutcomeStatus};
pub use memo::{generate_memo, FrameworkMemoSection};
pub use proposal::build_proposal;
