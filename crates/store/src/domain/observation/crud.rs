use worker::wasm_bindgen::JsValue;

use crate::{NewObservation, Observation, StoreError};

impl crate::D1Store {
    pub async fn create_observation(&self, o: &NewObservation) -> Result<i64, StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self
            .db
            .prepare("INSERT INTO observations (source_type, source_id, title, summary, content_hash, observed_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) RETURNING id")
            .bind(&[
                o.source_type.as_str().into(),
                o.source_id.as_str().into(),
                o.title.as_str().into(),
                o.summary.as_deref().unwrap_or("").into(),
                o.content_hash.as_deref().map_or(JsValue::null(), |v| v.into()),
                JsValue::from_f64(o.observed_at as f64),
                JsValue::from_f64(now as f64),
            ])?
            .first::<serde_json::Value>(None)
            .await?;
        row.and_then(|v| v["id"].as_i64()).ok_or_else(|| StoreError::D1("create_observation failed".into()))
    }

    pub async fn get_observation(&self, id: i64) -> Result<Option<Observation>, StoreError> {
        Ok(self
            .db
            .prepare("SELECT id, source_type, source_id, title, summary, content_hash, observed_at, created_at FROM observations WHERE id = ?1")
            .bind(&[JsValue::from_f64(id as f64)])?
            .first::<Observation>(None)
            .await?)
    }

    pub async fn find_observation_by_hash(&self, hash: &str) -> Result<Option<Observation>, StoreError> {
        Ok(self
            .db
            .prepare("SELECT id, source_type, source_id, title, summary, content_hash, observed_at, created_at FROM observations WHERE content_hash = ?1")
            .bind(&[hash.into()])?
            .first::<Observation>(None)
            .await?)
    }
}
