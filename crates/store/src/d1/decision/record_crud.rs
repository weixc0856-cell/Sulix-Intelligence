//! Decision Record and Outcome CRUD — Sprint 6.0 Decision Loop.

use crate::s_err::StoreResultExt;
use worker::wasm_bindgen::JsValue;

use crate::{DecisionOutcome, DecisionRecord, NewDecisionRecord, NewOutcome, StoreError};

impl crate::D1Store {
    /// Create a decision record.
    pub async fn create_decision_record(&self, d: &NewDecisionRecord) -> Result<i64, StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self
            .db
            .prepare(
                "INSERT INTO decision_records (title, context, decision_type, action, rationale, confidence, status, signal_id, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'active', ?7, ?8, ?9) RETURNING id",
            )
            .bind(&[
                d.title.as_str().into(),
                d.context.as_deref().map_or(JsValue::null(), |v| v.into()),
                d.decision_type.as_deref().map_or(JsValue::null(), |v| v.into()),
                d.action.as_deref().map_or(JsValue::null(), |v| v.into()),
                d.rationale.as_deref().map_or(JsValue::null(), |v| v.into()),
                JsValue::from_f64(d.confidence),
                d.signal_id.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                JsValue::from_f64(now as f64),
                JsValue::from_f64(now as f64),
            ]).s_err()?
            .first::<serde_json::Value>(None)
            .await.s_err()?;
        row.and_then(|v| v["id"].as_i64()).ok_or_else(|| StoreError::D1("create_decision_record failed".into()))
    }

    /// Get a decision record by id.
    pub async fn get_decision_record(&self, id: i64) -> Result<Option<DecisionRecord>, StoreError> {
        self.db
            .prepare(
                "SELECT id, title, context, decision_type, action, rationale, confidence, status, signal_id, memo_json, created_at, updated_at \
                 FROM decision_records WHERE id = ?1",
            )
            .bind(&[JsValue::from_f64(id as f64)]).s_err()?
            .first::<DecisionRecord>(None)
            .await
            .s_err()
    }

    /// List decision records by status.
    pub async fn list_decision_records(
        &self,
        status: Option<&str>,
        limit: u32,
    ) -> Result<Vec<DecisionRecord>, StoreError> {
        if let Some(s) = status {
            self.db
                .prepare("SELECT id, title, context, decision_type, action, rationale, confidence, status, signal_id, memo_json, created_at, updated_at \
                          FROM decision_records WHERE status = ?1 ORDER BY created_at DESC LIMIT ?2")
                .bind(&[s.into(), JsValue::from_f64(limit as f64)]).s_err()?
                .all().await.s_err()?.results()
                .s_err()
        } else {
            self.db
                .prepare("SELECT id, title, context, decision_type, action, rationale, confidence, status, signal_id, memo_json, created_at, updated_at \
                          FROM decision_records ORDER BY created_at DESC LIMIT ?1")
                .bind(&[JsValue::from_f64(limit as f64)]).s_err()?
                .all().await.s_err()?.results()
                .s_err()
        }
    }

    /// Delete a decision record.
    pub async fn delete_decision_record(&self, id: i64) -> Result<(), StoreError> {
        self.db
            .prepare("DELETE FROM decision_records WHERE id = ?1")
            .bind(&[JsValue::from_f64(id as f64)])
            .s_err()?
            .run()
            .await
            .s_err()?;
        Ok(())
    }

    /// Create an outcome for a decision.
    pub async fn create_outcome_metric(&self, o: &NewOutcome) -> Result<i64, StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self
            .db
            .prepare(
                "INSERT INTO decision_outcomes (decision_id, metric, expected_value, measurement_method, status, created_at) \
                 VALUES (?1, ?2, ?3, ?4, 'pending', ?5) RETURNING id",
            )
            .bind(&[
                JsValue::from_f64(o.decision_id as f64),
                o.metric.as_str().into(),
                o.expected_value.as_deref().map_or(JsValue::null(), |v| v.into()),
                o.measurement_method.as_deref().map_or(JsValue::null(), |v| v.into()),
                JsValue::from_f64(now as f64),
            ]).s_err()?
            .first::<serde_json::Value>(None)
            .await.s_err()?;
        row.and_then(|v| v["id"].as_i64()).ok_or_else(|| StoreError::D1("create_outcome_metric failed".into()))
    }

    /// List outcomes for a decision.
    pub async fn list_decision_outcomes(&self, decision_id: i64) -> Result<Vec<DecisionOutcome>, StoreError> {
        self.db
            .prepare("SELECT id, decision_id, metric, expected_value, actual_value, measurement_method, status, observed_at, created_at \
                      FROM decision_outcomes WHERE decision_id = ?1 ORDER BY id")
            .bind(&[JsValue::from_f64(decision_id as f64)]).s_err()?
            .all().await.s_err()?.results()
            .s_err()
    }

    /// Update outcome with actual value.
    pub async fn update_outcome_actual(&self, outcome_id: i64, actual: &str, achieved: bool) -> Result<(), StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let status = if achieved { "achieved" } else { "missed" };
        self.db
            .prepare("UPDATE decision_outcomes SET actual_value = ?1, status = ?2, observed_at = ?3 WHERE id = ?4")
            .bind(&[actual.into(), status.into(), JsValue::from_f64(now as f64), JsValue::from_f64(outcome_id as f64)])
            .s_err()?
            .run()
            .await
            .s_err()?;
        Ok(())
    }

    /// Link a claim to a decision record.
    pub async fn link_claim_to_decision(
        &self,
        decision_id: i64,
        claim_id: i64,
        relationship: &str,
    ) -> Result<(), StoreError> {
        self.db
            .prepare("INSERT OR REPLACE INTO decision_record_claims (decision_id, claim_id, relationship) VALUES (?1, ?2, ?3)")
            .bind(&[
                JsValue::from_f64(decision_id as f64),
                JsValue::from_f64(claim_id as f64),
                relationship.into(),
            ]).s_err()?
            .run().await.s_err()?;
        Ok(())
    }

    /// Get claims linked to a decision.
    pub async fn get_decision_claims(&self, decision_id: i64) -> Result<Vec<serde_json::Value>, StoreError> {
        self.db
            .prepare(
                "SELECT drc.decision_id, drc.claim_id, drc.relationship, c.statement AS claim_statement, c.claim_type \
                 FROM decision_record_claims drc LEFT JOIN claims c ON c.id = drc.claim_id \
                 WHERE drc.decision_id = ?1",
            )
            .bind(&[JsValue::from_f64(decision_id as f64)])
            .s_err()?
            .all()
            .await
            .s_err()?
            .results()
            .s_err()
    }

    /// Update memo_json on a decision record.
    pub async fn set_decision_memo(&self, id: i64, memo_json: &str) -> Result<(), StoreError> {
        self.db
            .prepare("UPDATE decision_records SET memo_json = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&[memo_json.into(), JsValue::from_f64(js_sys::Date::now() / 1000.0), JsValue::from_f64(id as f64)])
            .s_err()?
            .run()
            .await
            .s_err()?;
        Ok(())
    }

    /// Get reasoning framework traces for all claims linked to a decision.
    pub async fn get_decision_framework_traces(&self, decision_id: i64) -> Result<Vec<serde_json::Value>, StoreError> {
        self.db
            .prepare(
                "SELECT DISTINCT crf.framework_id, rf.name, rf.category, crf.relevance, crf.reasoning \
                 FROM decision_record_claims drc \
                 JOIN claim_reasoning_frameworks crf ON crf.claim_id = drc.claim_id \
                 JOIN reasoning_frameworks rf ON rf.id = crf.framework_id \
                 WHERE drc.decision_id = ?1 \
                 ORDER BY crf.relevance DESC",
            )
            .bind(&[JsValue::from_f64(decision_id as f64)])
            .s_err()?
            .all()
            .await
            .s_err()?
            .results()
            .s_err()
    }
}
