//! Outcome Events — factual observations recorded after a decision.
//!
//! This is the **fact layer** only. Judgments about whether the outcome
//! confirms or contradicts the hypothesis belong in `evaluation.rs`
//! (Sprint 3.3).

use crate::s_err::StoreResultExt;
use worker::wasm_bindgen::JsValue;

use crate::{NewOutcomeEvent, OutcomeEvent};

impl crate::D1Store {
    /// Record a factual outcome observation for a decision.
    pub async fn create_outcome(&self, e: &NewOutcomeEvent) -> Result<i64, crate::StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let observed_at = e.observed_at.unwrap_or(now);
        let row = self
            .db
            .prepare(
                "INSERT INTO outcome_events \
                 (decision_id, outcome_type, observation, evidence_url, observed_at, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6) RETURNING id",
            )
            .bind(&[
                JsValue::from_f64(e.decision_id as f64),
                e.outcome_type.as_str().into(),
                e.observation.as_str().into(),
                e.evidence_url.as_deref().map_or(JsValue::null(), |s| s.into()),
                JsValue::from_f64(observed_at as f64),
                JsValue::from_f64(now as f64),
            ])
            .s_err()?
            .first::<serde_json::Value>(None)
            .await
            .s_err()?;
        row.and_then(|v| v["id"].as_i64()).ok_or_else(|| crate::StoreError::D1("create_outcome failed".into()))
    }

    /// List factual outcome observations for a decision, newest first.
    pub async fn get_decision_outcomes(&self, decision_id: i64) -> Result<Vec<OutcomeEvent>, crate::StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT id, decision_id, outcome_type, observation, evidence_url, observed_at, created_at \
                 FROM outcome_events \
                 WHERE decision_id = ?1 \
                 ORDER BY observed_at DESC",
            )
            .bind(&[JsValue::from_f64(decision_id as f64)])
            .s_err()?
            .all()
            .await
            .s_err()?
            .results()
            .s_err()?)
    }
}
