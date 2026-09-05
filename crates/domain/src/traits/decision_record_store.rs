use async_trait::async_trait;

use crate::{DecisionOutcome, DecisionRecord, NewDecisionRecord, NewOutcome, StoreError};

/// Decision-Record persistence seam (D1 `decision_records` + related rows).
///
/// Covers both reads and the record's own writes (outcome-metric creation,
/// memo persistence) that form the "Verifiable Decision Record" sub-aggregate,
/// distinct from the GATED decision-write vertical (decision lifecycle /
/// engine events) which still goes through [`StoreBackend`](crate::StoreBackend).
///
/// Added in Phase 2 so the `/api/decision-records` read/memo use-cases ride a
/// narrow port instead of the concrete [`crate::D1Store`] inherent methods.
#[async_trait(?Send)]
pub trait DecisionRecordStore {
    /// Create a new decision record.  Returns the record id.
    async fn create_decision_record(&self, record: &NewDecisionRecord) -> Result<i64, StoreError>;

    /// Load a single decision record by primary key.
    async fn get_decision_record(&self, id: i64) -> Result<Option<DecisionRecord>, StoreError>;

    /// List decision records, optionally filtered by status.
    async fn list_decision_records(&self, status: Option<&str>, limit: u32) -> Result<Vec<DecisionRecord>, StoreError>;

    /// Create an outcome metric against a decision record.  Returns its id.
    async fn create_outcome_metric(&self, outcome: &NewOutcome) -> Result<i64, StoreError>;

    /// List outcome metrics for a decision record.
    async fn list_decision_outcomes(&self, decision_id: i64) -> Result<Vec<DecisionOutcome>, StoreError>;

    /// Claims linked to a decision record (loose row JSON).
    async fn get_decision_claims(&self, decision_id: i64) -> Result<Vec<serde_json::Value>, StoreError>;

    /// Persist a generated decision memo (JSON string).
    async fn set_decision_memo(&self, id: i64, memo_json: &str) -> Result<(), StoreError>;

    /// Reasoning-framework traces applied to a decision (loose row JSON).
    async fn get_decision_framework_traces(&self, decision_id: i64) -> Result<Vec<serde_json::Value>, StoreError>;
}
