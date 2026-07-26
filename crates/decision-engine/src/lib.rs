//! Decision Engine — Decision Intelligence Foundation for Sprint 6.0.
//!
//! Builds the action-feedback-learning loop:
//! Signal → Decision Proposal → Decision Record → Expected Outcome
//!                                                    ↓
//!                                            Actual Outcome
//!                                                    ↓
//!                                            Reflection → Calibration

pub mod domain;
pub mod events;
pub mod memo;
pub mod proposal;

pub use domain::{DecisionMemo, DecisionProposal, DecisionStatus, MemoSection, OutcomeStatus};
pub use memo::generate_memo;
pub use proposal::build_proposal;
