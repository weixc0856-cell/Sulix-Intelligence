use worker::wasm_bindgen::JsValue;

use crate::{Claim, NewClaim, StoreError};

impl crate::D1Store {
    pub async fn create_claim(&self, c: &NewClaim) -> Result<i64, StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self
            .db
            .prepare("INSERT INTO claims (statement, status, confidence, created_at, updated_at) VALUES (?1, ?2, 0.0, ?3, ?4) RETURNING id")
            .bind(&[
                c.statement.as_str().into(),
                c.status.as_deref().unwrap_or("active").into(),
                JsValue::from_f64(now as f64),
                JsValue::from_f64(now as f64),
            ])?
            .first::<serde_json::Value>(None)
            .await?;
        row.and_then(|v| v["id"].as_i64())
            .ok_or_else(|| StoreError::D1("create_claim failed".into()))
    }

    pub async fn get_claim(&self, id: i64) -> Result<Option<Claim>, StoreError> {
        Ok(self
            .db
            .prepare("SELECT id, statement, confidence, status, created_at, updated_at FROM claims WHERE id = ?1")
            .bind(&[JsValue::from_f64(id as f64)])?
            .first::<Claim>(None)
            .await?)
    }

    pub async fn list_claims(&self, status: Option<&str>, limit: u32) -> Result<Vec<Claim>, StoreError> {
        let sql = if status.is_some() {
            "SELECT id, statement, confidence, status, created_at, updated_at FROM claims WHERE status = ?1 ORDER BY created_at DESC LIMIT ?2"
        } else {
            "SELECT id, statement, confidence, status, created_at, updated_at FROM claims ORDER BY created_at DESC LIMIT ?1"
        };
        let stmt = self.db.prepare(sql);
        let stmt = if let Some(s) = status {
            stmt.bind(&[s.into(), JsValue::from_f64(limit as f64)])?
        } else {
            stmt.bind(&[JsValue::from_f64(limit as f64)])?
        };
        Ok(stmt.all().await?.results()?)
    }
}
