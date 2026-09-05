use async_trait::async_trait;

use crate::{NewDecisionEvaluation, StoreError};

/// Evaluation aggregate persistence.
///
/// Evaluation is a separate aggregate root, linked to Decision by `decision_id`.
/// Each evaluation records a judgment about whether a decision's hypothesis was correct.
#[async_trait(?Send)]
pub trait EvaluationRepository {
    /// Record a judgment about whether a decision's hypothesis was correct.
    /// Returns the evaluation id.
    async fn save_evaluation(&self, e: &NewDecisionEvaluation) -> Result<i64, StoreError>;
}
