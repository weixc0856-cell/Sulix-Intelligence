use async_trait::async_trait;

use crate::{NewDecision, NewDecisionEvaluation, NewOutcomeEvent, StoreError};

/// GATED decision-write vertical (decision lifecycle / engine events).
///
/// The four pre-Event-Sourcing write operations of the Decision aggregate,
/// held on a narrow port so the application-layer decision service can depend
/// on exactly what it calls instead of the full legacy
/// [`StoreBackend`](crate::StoreBackend) composite.
///
/// Signatures are verbatim from the legacy `StoreBackend` body (GATED:
/// mechanical relocation only — no SQL / write-contract / outbox-first change).
#[async_trait(?Send)]
pub trait DecisionWriteStore {
    /// Create a new decision.  Returns the decision id.
    async fn create_decision(&self, d: &NewDecision) -> Result<i64, StoreError>;

    /// Update a decision's lifecycle status.
    async fn update_decision_status(&self, id: i64, status: &str) -> Result<(), StoreError>;

    /// Record a factual outcome observation.  Returns the outcome id.
    async fn create_outcome(&self, e: &NewOutcomeEvent) -> Result<i64, StoreError>;

    /// Record a judgment about whether a decision's hypothesis was correct.
    /// Returns the evaluation id.
    async fn create_evaluation(&self, e: &NewDecisionEvaluation) -> Result<i64, StoreError>;
}
