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
