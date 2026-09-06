//! Read-model queries for the Decision Evaluation domain.
//!
//! Evaluations are created by [`super::super::repo::EvaluationRepository`].
//! `list_evaluations` was historically `get_decision_evaluations` on
//! `StoreBackend`; the separate `get_latest_evaluation` read was retired as
//! dead code (2026-09-06).

use async_trait::async_trait;

use crate::{DecisionEvaluation, StoreError};

#[async_trait(?Send)]
pub trait EvaluationQueryService {
    /// List all evaluations for a decision, newest first.
    async fn list_evaluations(&self, decision_id: i64) -> Result<Vec<DecisionEvaluation>, StoreError>;
}
