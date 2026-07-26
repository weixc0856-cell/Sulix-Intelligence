use async_trait::async_trait;

use crate::{Decision, NewDecision, StoreError};

/// Decision aggregate persistence.
///
/// Manages the decision record (hypothesis, confidence, status).
/// Outcomes and evaluations are written through
/// [`super::super::backend::StoreBackend`] until event sourcing is formalised.
/// Read-model queries (stats, outcomes, evaluations) belong in
/// [`super::super::query::DecisionQueryService`].
#[async_trait(?Send)]
pub trait DecisionRepository {
    /// Create a new decision.  Returns the decision id.
    async fn save_decision(&self, decision: &NewDecision) -> Result<i64, StoreError>;

    /// Load a decision by its primary key.
    async fn find_decision(&self, id: i64) -> Result<Option<Decision>, StoreError>;
}
