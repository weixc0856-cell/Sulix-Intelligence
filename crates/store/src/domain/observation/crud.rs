use worker::wasm_bindgen::JsValue;

use crate::{NewObservation, Observation, StoreError};

impl crate::D1Store {
    pub async fn create_observation(&self, o: &NewObservation) -> Result<i64, StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self
            .db
            .prepare(
                "INSERT INTO observations (source_type, source_id, title, summary, content_hash, \
                 url, article_id, registry_source_id, observed_at, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) RETURNING id",
            )
            .bind(&[
                o.source_type.as_str().into(),
                o.source_id.as_str().into(),
                o.title.as_str().into(),
                o.summary.as_deref().unwrap_or("").into(),
                o.content_hash.as_deref().map_or(JsValue::null(), |v| v.into()),
                o.url.as_deref().map_or(JsValue::null(), |v| v.into()),
                o.article_id.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                o.registry_source_id.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                JsValue::from_f64(o.observed_at as f64),
                JsValue::from_f64(now as f64),
            ])?
            .first::<serde_json::Value>(None)
            .await?;
        row.and_then(|v| v["id"].as_i64()).ok_or_else(|| StoreError::D1("create_observation failed".into()))
    }

    pub async fn get_observation(&self, id: i64) -> Result<Option<Observation>, StoreError> {
        self.db
            .prepare(
                "SELECT id, source_type, source_id, title, summary, content_hash, \
                 url, article_id, registry_source_id, observed_at, created_at \
                 FROM observations WHERE id = ?1",
            )
            .bind(&[JsValue::from_f64(id as f64)])?
            .first::<Observation>(None)
            .await
            .map_err(StoreError::from)
    }

    pub async fn find_observation_by_hash(&self, hash: &str) -> Result<Option<Observation>, StoreError> {
        self.db
            .prepare(
                "SELECT id, source_type, source_id, title, summary, content_hash, \
                 url, article_id, registry_source_id, observed_at, created_at \
                 FROM observations WHERE content_hash = ?1",
            )
            .bind(&[hash.into()])?
            .first::<Observation>(None)
            .await
            .map_err(StoreError::from)
    }

    /// List observations with optional source filter, paginated.
    pub async fn list_observations(
        &self,
        source_type: Option<&str>,
        source_id: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Observation>, StoreError> {
        let mut sql = String::from(
            "SELECT id, source_type, source_id, title, summary, content_hash, \
             url, article_id, registry_source_id, observed_at, created_at \
             FROM observations WHERE 1=1",
        );
        let mut params: Vec<JsValue> = Vec::new();
        let mut idx = 1;

        if let Some(st) = source_type {
            sql.push_str(&format!(" AND source_type = ?{idx}"));
            params.push(st.into());
            idx += 1;
        }
        if let Some(sid) = source_id {
            sql.push_str(&format!(" AND source_id = ?{idx}"));
            params.push(sid.into());
            idx += 1;
        }

        sql.push_str(" ORDER BY observed_at DESC");
        sql.push_str(&format!(" LIMIT ?{idx}"));
        params.push(JsValue::from_f64(limit as f64));
        idx += 1;
        sql.push_str(&format!(" OFFSET ?{idx}"));
        params.push(JsValue::from_f64(offset as f64));

        Ok(self.db.prepare(&sql).bind(&params)?.all().await?.results()?)
    }
}
