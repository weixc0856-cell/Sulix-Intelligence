use crate::s_err::StoreResultExt;
use worker::wasm_bindgen::JsValue;

use crate::{NewSource, Source, StoreError};

impl crate::D1Store {
    /// Upsert a source entry (insert or update on feed_id conflict).
    /// Returns the source id.
    pub async fn save_source(&self, s: &NewSource) -> Result<i64, StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self
            .db
            .prepare(
                "INSERT INTO sources (source_type, feed_id, name, tier, policy, license, \
                 license_detail, attribution, trust_score, retention_days, verified, notes, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13) \
                 ON CONFLICT(feed_id) DO UPDATE SET \
                    source_type = COALESCE(?1, source_type), \
                    name = COALESCE(?3, name), \
                    tier = COALESCE(?4, tier), \
                    policy = COALESCE(?5, policy), \
                    license = COALESCE(?6, license), \
                    license_detail = COALESCE(?7, license_detail), \
                    attribution = COALESCE(?8, attribution), \
                    trust_score = COALESCE(?9, trust_score), \
                    retention_days = COALESCE(?10, retention_days), \
                    verified = COALESCE(?11, verified), \
                    notes = COALESCE(?12, notes), \
                    updated_at = ?13 \
                 RETURNING id",
            )
            .bind(&[
                s.source_type.as_str().into(),
                s.feed_id.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                s.name.as_deref().map_or(JsValue::null(), |v| v.into()),
                s.tier.as_str().into(),
                s.policy.as_str().into(),
                s.license.as_str().into(),
                s.license_detail.as_deref().map_or(JsValue::null(), |v| v.into()),
                s.attribution.as_deref().map_or(JsValue::null(), |v| v.into()),
                s.trust_score.map_or(JsValue::null(), JsValue::from_f64),
                s.retention_days.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                JsValue::from_f64(if s.verified { 1.0 } else { 0.0 }),
                s.notes.as_deref().map_or(JsValue::null(), |v| v.into()),
                JsValue::from_f64(now as f64),
            ])
            .s_err()?
            .first::<serde_json::Value>(None)
            .await
            .s_err()?;
        row.and_then(|v| v["id"].as_i64()).ok_or_else(|| StoreError::D1("save_source failed".into()))
    }

    /// Get a source by its primary key.
    pub async fn find_source(&self, id: i64) -> Result<Option<Source>, StoreError> {
        self.db
            .prepare(
                "SELECT id, source_type, feed_id, name, tier, policy, license, license_detail, \
                 attribution, trust_score, retention_days, verified, notes, created_at, updated_at \
                 FROM sources WHERE id = ?1",
            )
            .bind(&[JsValue::from_f64(id as f64)])
            .s_err()?
            .first::<Source>(None)
            .await
            .s_err()
    }

    /// Get a source by feed_id (1:1 relationship).
    pub async fn find_source_by_feed(&self, feed_id: i64) -> Result<Option<Source>, StoreError> {
        self.db
            .prepare(
                "SELECT id, source_type, feed_id, name, tier, policy, license, license_detail, \
                 attribution, trust_score, retention_days, verified, notes, created_at, updated_at \
                 FROM sources WHERE feed_id = ?1",
            )
            .bind(&[JsValue::from_f64(feed_id as f64)])
            .s_err()?
            .first::<Source>(None)
            .await
            .s_err()
    }

    /// Delete a source entry by id.
    pub async fn delete_source(&self, id: i64) -> Result<(), StoreError> {
        self.db
            .prepare("DELETE FROM sources WHERE id = ?1")
            .bind(&[JsValue::from_f64(id as f64)])
            .s_err()?
            .run()
            .await
            .s_err()?;
        Ok(())
    }

    /// List sources with optional tier/policy filters.
    pub async fn list_sources(
        &self,
        tier: Option<&str>,
        policy: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Source>, StoreError> {
        let mut sql = String::from(
            "SELECT id, source_type, feed_id, name, tier, policy, license, license_detail, \
             attribution, trust_score, retention_days, verified, notes, created_at, updated_at \
             FROM sources WHERE 1=1",
        );
        let mut params: Vec<JsValue> = Vec::new();
        let mut idx = 1;

        if let Some(t) = tier {
            sql.push_str(&format!(" AND tier = ?{idx}"));
            params.push(t.into());
            idx += 1;
        }
        if let Some(p) = policy {
            sql.push_str(&format!(" AND policy = ?{idx}"));
            params.push(p.into());
            idx += 1;
        }

        sql.push_str(" ORDER BY id DESC");
        sql.push_str(&format!(" LIMIT ?{idx}"));
        params.push(JsValue::from_f64(limit as f64));
        idx += 1;
        sql.push_str(&format!(" OFFSET ?{idx}"));
        params.push(JsValue::from_f64(offset as f64));

        Ok(self.db.prepare(&sql).bind(&params).s_err()?.all().await.s_err()?.results().s_err()?)
    }
}
