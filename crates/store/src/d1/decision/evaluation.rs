//! Decision Evaluation — Judgment Layer for the Decision Loop.
//!
//! This is separate from OutcomeObservation (fact layer).
//! An evaluation answers: "was the hypothesis confirmed by reality?"

use crate::s_err::StoreResultExt;
use worker::wasm_bindgen::JsValue;

use crate::{DecisionEvaluation, NewDecisionEvaluation};

impl crate::D1Store {
    /// Record a judgment about whether a decision's hypothesis was correct
    /// (backing the `EvaluationRepository::save_evaluation` port).
    pub async fn insert_decision_evaluation(&self, e: &NewDecisionEvaluation) -> Result<i64, crate::StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let evaluated_at = e.evaluated_at.unwrap_or(now);
        let row = self
            .db
            .prepare(
                "INSERT INTO decision_evaluations \
                 (decision_id, evaluation, confidence, reasoning, evaluator, evaluated_at, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) RETURNING id",
            )
            .bind(&[
                JsValue::from_f64(e.decision_id as f64),
                e.evaluation.to_string().into(),
                e.confidence.map_or(JsValue::null(), JsValue::from_f64),
                e.reasoning.as_deref().map_or(JsValue::null(), |s| s.into()),
                e.evaluator.to_string().into(),
                JsValue::from_f64(evaluated_at as f64),
                JsValue::from_f64(now as f64),
            ])
            .s_err()?
            .first::<serde_json::Value>(None)
            .await
            .s_err()?;
        row.and_then(|v| v["id"].as_i64())
            .ok_or_else(|| crate::StoreError::D1("insert_decision_evaluation failed".into()))
    }

    /// List all evaluations for a decision, newest first.
    pub async fn get_decision_evaluations(
        &self,
        decision_id: i64,
    ) -> Result<Vec<DecisionEvaluation>, crate::StoreError> {
        self.db
            .prepare(
                "SELECT id, decision_id, evaluation, confidence, reasoning, evaluator, evaluated_at, created_at \
                 FROM decision_evaluations \
                 WHERE decision_id = ?1 \
                 ORDER BY created_at DESC",
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
