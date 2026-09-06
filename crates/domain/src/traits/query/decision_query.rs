//! Read-model queries for the Decision Loop domain.
//!
//! Decision-row reads (`find_decision`) belong in
//! [`super::super::repo::DecisionRepository`]; the row write goes through
//! `DecisionUpsertStore` (decision-engine vertical).  Outcome and evaluation
//! reads live here alongside the decision stats.

use async_trait::async_trait;

use crate::{Decision, DecisionEvaluation, DecisionStats, OutcomeEvent, StoreError};

#[async_trait(?Send)]
pub trait DecisionQueryService {
    /// List decisions, optionally filtered by status.
    async fn list_decisions(&self, status: Option<&str>, limit: u32) -> Result<Vec<Decision>, StoreError>;

    /// List decisions for a specific signal thread.
    async fn decisions_by_signal(&self, signal_thread_id: i64) -> Result<Vec<Decision>, StoreError>;

    /// Aggregated decision statistics for the dashboard.
    async fn decision_stats(&self) -> Result<DecisionStats, StoreError>;

    /// List outcome observations for a decision.
    async fn list_outcomes(&self, decision_id: i64) -> Result<Vec<OutcomeEvent>, StoreError>;

    /// List all evaluations for a decision, newest first.
    async fn list_evaluations(&self, decision_id: i64) -> Result<Vec<DecisionEvaluation>, StoreError>;
}
