//! Read-model queries for the Decision Evaluation domain.
//!
//! Evaluations are created by [`super::super::repo::EvaluationRepository`].
//! These read methods were historically on `StoreBackend` as
//! `get_decision_evaluations` / `get_latest_evaluation`.

use async_trait::async_trait;

use crate::{DecisionEvaluation, StoreError};

#[async_trait(?Send)]
pub trait EvaluationQueryService {
    /// List all evaluations for a decision, newest first.
    async fn list_evaluations(&self, decision_id: i64) -> Result<Vec<DecisionEvaluation>, StoreError>;

    /// Get the latest evaluation for a decision.
    async fn get_latest_evaluation(&self, decision_id: i64) -> Result<Option<DecisionEvaluation>, StoreError>;
}
