//! Decision queries — list decisions by status or signal thread.

use crate::s_err::StoreResultExt;
use worker::wasm_bindgen::JsValue;

use crate::Decision;

impl crate::D1Store {
    /// List decisions, optionally filtered by status.
    pub async fn list_decisions(&self, status: Option<&str>, limit: u32) -> Result<Vec<Decision>, crate::StoreError> {
        match status {
            Some(s) => Ok(self
                .db
                .prepare(
                    "SELECT id, signal_thread_id, actor_id, decision_type, title, hypothesis, rationale, \
                                confidence, status, priority, expected_outcomes, created_at, updated_at \
                         FROM decisions WHERE status = ?1 ORDER BY created_at DESC LIMIT ?2",
                )
                .bind(&[s.into(), JsValue::from_f64(limit as f64)])
                .s_err()?
                .all()
                .await
                .s_err()?
                .results()
                .s_err()?),
            None => Ok(self
                .db
                .prepare(
                    "SELECT id, signal_thread_id, actor_id, decision_type, title, hypothesis, rationale, \
                                confidence, status, priority, expected_outcomes, created_at, updated_at \
                         FROM decisions ORDER BY created_at DESC LIMIT ?1",
                )
                .bind(&[JsValue::from_f64(limit as f64)])
                .s_err()?
                .all()
                .await
                .s_err()?
                .results()
                .s_err()?),
        }
    }

    /// List decisions for a specific signal thread.
    pub async fn decisions_by_signal(&self, signal_thread_id: i64) -> Result<Vec<Decision>, crate::StoreError> {
        self.db
            .prepare(
                "SELECT id, signal_thread_id, actor_id, decision_type, title, hypothesis, rationale, \
                        confidence, status, priority, expected_outcomes, created_at, updated_at \
                 FROM decisions WHERE signal_thread_id = ?1 ORDER BY created_at DESC LIMIT 50",
            )
            .bind(&[JsValue::from_f64(signal_thread_id as f64)])
            .s_err()?
            .all()
            .await
            .s_err()?
            .results()
            .s_err()
    }

    /// Get aggregated decision statistics.
    pub async fn decision_stats(&self) -> Result<crate::DecisionStats, crate::StoreError> {
        let status_row: Option<serde_json::Value> = self
            .db
            .prepare(
                "SELECT \
                 COUNT(*) AS total, \
                 SUM(CASE WHEN status = 'active' THEN 1 ELSE 0 END) AS active, \
                 SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) AS completed, \
                 SUM(CASE WHEN status = 'superseded' THEN 1 ELSE 0 END) AS superseded \
                 FROM decisions",
            )
            .bind(&[])
            .s_err()?
            .first::<serde_json::Value>(None)
            .await
            .s_err()?;
        let sv = status_row.unwrap_or_default();
        let total = sv["total"].as_i64().unwrap_or(0);
        let active = sv["active"].as_i64().unwrap_or(0);
        let completed = sv["completed"].as_i64().unwrap_or(0);
        let superseded = sv["superseded"].as_i64().unwrap_or(0);

        let by_type: Vec<crate::TypeCount> = self
            .db
            .prepare("SELECT decision_type AS label, COUNT(*) AS count FROM decisions GROUP BY decision_type ORDER BY count DESC")
            .bind(&[]).s_err()?
            .all()
            .await.s_err()?
            .results().s_err()?;

        let by_priority: Vec<crate::PriorityCount> = self
            .db
            .prepare("SELECT priority AS label, COUNT(*) AS count FROM decisions GROUP BY priority ORDER BY count DESC")
            .bind(&[])
            .s_err()?
            .all()
            .await
            .s_err()?
            .results()
            .s_err()?;

        let eval_row: Option<serde_json::Value> = self
            .db
            .prepare(
                "SELECT \
                 COUNT(*) AS total, \
                 SUM(CASE WHEN evaluation = 'confirmed' THEN 1 ELSE 0 END) AS confirmed, \
                 SUM(CASE WHEN evaluation = 'partially_confirmed' THEN 1 ELSE 0 END) AS partially, \
                 SUM(CASE WHEN evaluation = 'contradicted' THEN 1 ELSE 0 END) AS contradicted, \
                 SUM(CASE WHEN evaluation = 'inconclusive' THEN 1 ELSE 0 END) AS inconclusive \
                 FROM decision_evaluations",
            )
            .bind(&[])
            .s_err()?
            .first::<serde_json::Value>(None)
            .await
            .s_err()?;
        let ev = eval_row.unwrap_or_default();
        let eval_total = ev["total"].as_i64().unwrap_or(0);
        let confirmed = ev["confirmed"].as_i64().unwrap_or(0);
        let partially = ev["partially"].as_i64().unwrap_or(0);
        let contradicted = ev["contradicted"].as_i64().unwrap_or(0);
        let inconclusive = ev["inconclusive"].as_i64().unwrap_or(0);
        let accuracy_rate = if eval_total > inconclusive {
            let numerator = confirmed as f64 + partially as f64 * 0.5;
            let denominator = (eval_total - inconclusive) as f64;
            if denominator > 0.0 {
                numerator / denominator
            } else {
                0.0
            }
        } else {
            0.0
        };

        let top_signals: Vec<crate::SignalDecisionCount> = self
            .db
            .prepare(
                "SELECT d.signal_thread_id AS signal_id, COALESCE(t.title, '') AS signal_title, COUNT(*) AS decision_count \
                 FROM decisions d \
                 LEFT JOIN signal_threads t ON t.id = d.signal_thread_id \
                 WHERE d.signal_thread_id IS NOT NULL \
                 GROUP BY d.signal_thread_id \
                 ORDER BY decision_count DESC LIMIT 10",
            )
            .bind(&[]).s_err()?
            .all()
            .await.s_err()?
            .results().s_err()?;

        Ok(crate::DecisionStats {
            total_decisions: total,
            active,
            completed,
            superseded,
            by_type,
            by_priority,
            evaluation_summary: crate::EvalSummary {
                total_evaluated: eval_total,
                confirmed,
                partially_confirmed: partially,
                contradicted,
                inconclusive,
                accuracy_rate: (accuracy_rate * 1000.0).round() / 1000.0,
            },
            top_signals,
        })
    }
}
