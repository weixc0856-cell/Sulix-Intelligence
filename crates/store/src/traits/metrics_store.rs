use async_trait::async_trait;

use crate::StoreError;

/// Trust / model-metrics seam (D1 `reasoning_runs`,
/// `decision_records`, `decision_outcomes`, `confidence_calibrations`).
///
/// Added in Phase 2 so the `/api/.../trust` use-case rides a narrow port
/// instead of inherent `D1Store` methods.  The stats are exposed as raw row
/// JSON — no typed aggregate models exist for them yet.
#[async_trait(?Send)]
pub trait MetricsStore {
    /// Per (reasoning_type, model_name) invocation stats from `reasoning_runs`.
    async fn model_reliability_stats(&self) -> Result<Vec<serde_json::Value>, StoreError>;

    /// Decision-record accuracy aggregate (single object).
    async fn decision_accuracy_stats(&self) -> Result<serde_json::Value, StoreError>;

    /// Outcome success-rate aggregate (single object).
    async fn outcome_success_stats(&self) -> Result<serde_json::Value, StoreError>;

    /// Per-entity-type calibration-error aggregate.
    async fn calibration_stats(&self) -> Result<Vec<serde_json::Value>, StoreError>;
}
