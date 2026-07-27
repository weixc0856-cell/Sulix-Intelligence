//! Decision lifecycle state machine.
//!
//! Transitions form a DAG, not a free-for-all:
//!
//! ```text
//! Draft ──→ Proposed ──→ Approved ──→ Executing ──→ Completed
//!              │                          │
//!              └──→ Invalidated ←─────────┘
//! ```
//!
//! Each transition is guarded by pre-conditions checked by the
//! `DecisionAggregate`.

use serde::{Deserialize, Serialize};

/// Domain-safe lifecycle status for a Decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    /// Initial state — being drafted, not yet actionable.
    Draft,
    /// Proposed for review — ready for approval or rejection.
    Proposed,
    /// Approved — cleared for execution.
    Approved,
    /// Being acted upon.
    Executing,
    /// Execution complete — all outcomes observed.
    Completed,
    /// Invalidated — superseded, withdrawn, or no longer relevant.
    Invalidated,
}

impl DecisionStatus {
    /// Returns `true` if a transition from `self` to `target` is allowed
    /// by the state machine.
    pub fn can_transition_to(&self, target: &Self) -> bool {
        matches!(
            (self, target),
            // Draft → Proposed (submit for review)
            (Self::Draft, Self::Proposed)
            // Proposed → Approved | Invalidated (review outcome)
            | (Self::Proposed, Self::Approved)
            | (Self::Proposed, Self::Invalidated)
            // Approved → Executing (start execution)
            | (Self::Approved, Self::Executing)
            // Executing → Completed | Invalidated (finish or abort)
            | (Self::Executing, Self::Completed)
            | (Self::Executing, Self::Invalidated)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_can_be_proposed() {
        assert!(DecisionStatus::Draft.can_transition_to(&DecisionStatus::Proposed));
    }

    #[test]
    fn draft_cannot_skip_to_approved() {
        assert!(!DecisionStatus::Draft.can_transition_to(&DecisionStatus::Approved));
    }

    #[test]
    fn draft_cannot_complete() {
        assert!(!DecisionStatus::Draft.can_transition_to(&DecisionStatus::Completed));
    }

    #[test]
    fn proposed_can_be_approved() {
        assert!(DecisionStatus::Proposed.can_transition_to(&DecisionStatus::Approved));
    }

    #[test]
    fn proposed_can_be_invalidated() {
        assert!(DecisionStatus::Proposed.can_transition_to(&DecisionStatus::Invalidated));
    }

    #[test]
    fn proposed_cannot_be_executing() {
        assert!(!DecisionStatus::Proposed.can_transition_to(&DecisionStatus::Executing));
    }

    #[test]
    fn executing_can_complete() {
        assert!(DecisionStatus::Executing.can_transition_to(&DecisionStatus::Completed));
    }

    #[test]
    fn executing_can_be_invalidated() {
        assert!(DecisionStatus::Executing.can_transition_to(&DecisionStatus::Invalidated));
    }

    #[test]
    fn completed_cannot_transition() {
        assert!(!DecisionStatus::Completed.can_transition_to(&DecisionStatus::Draft));
        assert!(!DecisionStatus::Completed.can_transition_to(&DecisionStatus::Proposed));
        assert!(!DecisionStatus::Completed.can_transition_to(&DecisionStatus::Approved));
    }

    #[test]
    fn invalidated_cannot_transition() {
        assert!(!DecisionStatus::Invalidated.can_transition_to(&DecisionStatus::Proposed));
        assert!(!DecisionStatus::Invalidated.can_transition_to(&DecisionStatus::Completed));
        assert!(!DecisionStatus::Invalidated.can_transition_to(&DecisionStatus::Draft));
    }
}
