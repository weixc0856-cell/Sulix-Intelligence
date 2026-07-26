//! Intelligence metrics queries — model reliability, calibration, reasoning stats.

use crate::StoreError;

impl crate::D1Store {
    /// Query model invocation statistics from reasoning_runs table.
    pub async fn model_reliability_stats(&self) -> Result<Vec<serde_json::Value>, StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT reasoning_type, model_name, \
                 COUNT(*) AS total_calls, \
                 ROUND(AVG(CASE WHEN success = 1 THEN 1.0 ELSE 0.0 END), 4) AS success_rate, \
                 ROUND(AVG(latency_ms), 0) AS avg_latency, \
                 ROUND(AVG(output_tokens), 0) AS avg_output_tokens \
                 FROM reasoning_runs \
                 GROUP BY reasoning_type, model_name \
                 ORDER BY total_calls DESC",
            )
            .bind(&[])?
            .all()
            .await?
            .results()?)
    }

    /// Query decision record accuracy stats.
    pub async fn decision_accuracy_stats(&self) -> Result<serde_json::Value, StoreError> {
        self.db
            .prepare(
                "SELECT \
                COUNT(*) AS total_records, \
                SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) AS completed_count, \
                ROUND(AVG(confidence), 4) AS avg_confidence \
                FROM decision_records",
            )
            .bind(&[])?
            .first::<serde_json::Value>(None)
            .await
            .map(|opt| opt.unwrap_or_default())
            .map_err(StoreError::from)
    }

    /// Query outcome success rate.
    pub async fn outcome_success_stats(&self) -> Result<serde_json::Value, StoreError> {
        self.db
            .prepare(
                "SELECT \
                COUNT(*) AS total_outcomes, \
                ROUND(AVG(CASE WHEN status = 'achieved' THEN 1.0 ELSE 0.0 END), 4) AS success_rate \
                FROM decision_outcomes WHERE status IN ('achieved', 'missed')",
            )
            .bind(&[])?
            .first::<serde_json::Value>(None)
            .await
            .map(|opt| opt.unwrap_or_default())
            .map_err(StoreError::from)
    }

    /// Query calibration statistics from confidence_calibrations table.
    pub async fn calibration_stats(&self) -> Result<Vec<serde_json::Value>, StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT entity_type, \
                 COUNT(*) AS total_predictions, \
                 ROUND(AVG(calibration_error), 4) AS avg_calibration_error, \
                 ROUND(AVG(predicted_confidence), 4) AS avg_confidence, \
                 ROUND(AVG(COALESCE(actual_outcome, 0.0)), 4) AS avg_actual \
                 FROM confidence_calibrations \
                 GROUP BY entity_type",
            )
            .bind(&[])?
            .all()
            .await?
            .results()?)
    }
}
