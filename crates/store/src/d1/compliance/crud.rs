use crate::s_err::StoreResultExt;
use worker::wasm_bindgen::JsValue;

use crate::StoreError;

impl crate::D1Store {
    /// Create a takedown request and optionally block the article.
    /// Returns the takedown request id.
    pub async fn create_takedown(
        &self,
        source_id: Option<i64>,
        article_id: Option<i64>,
        requester_email: &str,
        reason: &str,
    ) -> Result<i64, StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self
            .db
            .prepare(
                "INSERT INTO takedown_requests (source_id, article_id, requester_email, reason, status, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, 'submitted', ?5) RETURNING id",
            )
            .bind(&[
                source_id.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                article_id.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                requester_email.into(),
                reason.into(),
                JsValue::from_f64(now as f64),
            ])
            .s_err()?
            .first::<serde_json::Value>(None)
            .await
            .s_err()?;

        let takedown_id =
            row.and_then(|v| v["id"].as_i64()).ok_or_else(|| StoreError::D1("create_takedown failed".into()))?;

        // Auto-block article serving if article_id specified
        if let Some(aid) = article_id {
            let _ = self
                .db
                .prepare(
                    "INSERT INTO content_visibility_overrides (article_id, takedown_id, action) \
                     VALUES (?1, ?2, 'block_serve')",
                )
                .bind(&[JsValue::from_f64(aid as f64), JsValue::from_f64(takedown_id as f64)])
                .s_err()?
                .run()
                .await;
        }

        Ok(takedown_id)
    }

    /// List takedown requests with optional status filter.
    pub async fn list_takedowns(&self, status: Option<&str>, limit: u32) -> Result<Vec<serde_json::Value>, StoreError> {
        let mut sql = String::from(
            "SELECT id, source_id, article_id, requester_email, reason, status, notes, \
             created_at, updated_at, processed_at FROM takedown_requests WHERE 1=1",
        );
        let mut params: Vec<JsValue> = Vec::new();
        let mut idx = 1;

        if let Some(s) = status {
            sql.push_str(&format!(" AND status = ?{idx}"));
            params.push(s.into());
            idx += 1;
        }

        sql.push_str(&format!(" ORDER BY created_at DESC LIMIT ?{idx}"));
        params.push(JsValue::from_f64(limit as f64));

        Ok(self.db.prepare(&sql).bind(&params).s_err()?.all().await.s_err()?.results().s_err()?)
    }

    /// Update takedown request status (approve/reject/review).
    pub async fn update_takedown_status(&self, id: i64, status: &str, notes: Option<&str>) -> Result<(), StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        self.db
            .prepare(
                "UPDATE takedown_requests SET status = ?1, notes = COALESCE(?2, notes), \
                 processed_at = ?3, updated_at = ?3 WHERE id = ?4",
            )
            .bind(&[
                status.into(),
                notes.map_or(JsValue::null(), |v| v.into()),
                JsValue::from_f64(now as f64),
                JsValue::from_f64(id as f64),
            ])
            .s_err()?
            .run()
            .await
            .s_err()?;

        // If approved, ensure visibility overrides are active
        if status == "approved" {
            let _ = self
                .db
                .prepare("UPDATE content_visibility_overrides SET active = 1 WHERE takedown_id = ?1")
                .bind(&[JsValue::from_f64(id as f64)])
                .s_err()?
                .run()
                .await;
        }

        Ok(())
    }
}
