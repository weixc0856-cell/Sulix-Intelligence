//! Decision queries — list decisions by status or signal thread.

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
                                confidence, status, priority, created_at, updated_at \
                         FROM decisions WHERE status = ?1 ORDER BY created_at DESC LIMIT ?2",
                )
                .bind(&[s.into(), JsValue::from_f64(limit as f64)])?
                .all()
                .await?
                .results()?),
            None => Ok(self
                .db
                .prepare(
                    "SELECT id, signal_thread_id, actor_id, decision_type, title, hypothesis, rationale, \
                                confidence, status, priority, created_at, updated_at \
                         FROM decisions ORDER BY created_at DESC LIMIT ?1",
                )
                .bind(&[JsValue::from_f64(limit as f64)])?
                .all()
                .await?
                .results()?),
        }
    }

    /// List decisions for a specific signal thread.
    pub async fn decisions_by_signal(&self, signal_thread_id: i64) -> Result<Vec<Decision>, crate::StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT id, signal_thread_id, actor_id, decision_type, title, hypothesis, rationale, \
                        confidence, status, priority, created_at, updated_at \
                 FROM decisions WHERE signal_thread_id = ?1 ORDER BY created_at DESC LIMIT 50",
            )
            .bind(&[JsValue::from_f64(signal_thread_id as f64)])?
            .all()
            .await?
            .results()?)
    }
}
