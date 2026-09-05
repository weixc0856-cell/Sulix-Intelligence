use crate::s_err::StoreResultExt;
use worker::wasm_bindgen::JsValue;

use crate::{ConfidenceEvent, NewConfidenceEvent, StoreError};

impl crate::D1Store {
    /// Append a confidence event with optional factor explanations.
    pub async fn append_confidence(&self, e: &NewConfidenceEvent) -> Result<i64, StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;

        // Get latest confidence for this entity as previous_confidence
        let prev: Option<f64> = self
            .db
            .prepare("SELECT confidence FROM confidence_events WHERE entity_type = ?1 AND entity_id = ?2 ORDER BY created_at DESC LIMIT 1")
            .bind(&[e.entity_type.as_str().into(), e.entity_id.as_str().into()]).s_err()?
            .first::<serde_json::Value>(None)
            .await.s_err()?
            .and_then(|v| v["confidence"].as_f64());

        let row = self
            .db
            .prepare(
                "INSERT INTO confidence_events (entity_type, entity_id, previous_confidence, confidence, reason, trigger_event, factors_json, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) RETURNING id",
            )
            .bind(&[
                e.entity_type.as_str().into(),
                e.entity_id.as_str().into(),
                prev.map_or(JsValue::null(), JsValue::from_f64),
                JsValue::from_f64(e.confidence),
                e.reason.as_deref().map_or(JsValue::null(), |v| v.into()),
                e.trigger_event.as_deref().map_or(JsValue::null(), |v| v.into()),
                e.factors_json.as_deref().map_or(JsValue::null(), |v| v.into()),
                JsValue::from_f64(now as f64),
            ]).s_err()?
            .first::<serde_json::Value>(None)
            .await.s_err()?;
        row.and_then(|v| v["id"].as_i64()).ok_or_else(|| StoreError::D1("append_confidence failed".into()))
    }

    pub async fn list_confidence_history(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Vec<ConfidenceEvent>, StoreError> {
        self.db
            .prepare(
                "SELECT id, entity_type, entity_id, previous_confidence, confidence, reason, trigger_event, \
                 factors_json, created_at \
                 FROM confidence_events WHERE entity_type = ?1 AND entity_id = ?2 ORDER BY created_at ASC",
            )
            .bind(&[entity_type.into(), entity_id.into()])
            .s_err()?
            .all()
            .await
            .s_err()?
            .results()
            .s_err()
    }
}
