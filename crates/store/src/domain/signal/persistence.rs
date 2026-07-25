use worker::wasm_bindgen::JsValue;

impl crate::D1Store {
    /// Persist an intelligence signal with evidence and entity links.
    #[allow(clippy::too_many_arguments)]
    pub async fn save_signal(
        &self,
        entity_id: Option<i64>,
        title: &str,
        summary: &str,
        confidence: f64,
        impact: &str,
        trend: &str,
        article_count: i64,
        source_count: i64,
        evidence_ids: &[i64],
        related_ids: &[i64],
    ) -> Result<i64, crate::StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self
            .db
            .prepare(
                "INSERT INTO intelligence_signals \
                 (anchor_entity_id, title, summary, signal_type, confidence, impact, trend, article_count, source_count, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, 'entity', ?4, ?5, ?6, ?7, ?8, ?9, ?10) RETURNING id",
            )
            .bind(&[
                entity_id.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                title.into(), summary.into(), JsValue::from_f64(confidence),
                impact.into(), trend.into(),
                JsValue::from_f64(article_count as f64),
                JsValue::from_f64(source_count as f64),
                JsValue::from_f64(now as f64), JsValue::from_f64(now as f64),
            ])?
            .first::<serde_json::Value>(None)
            .await?;
        let signal_id = row.and_then(|v| v["id"].as_i64())
            .ok_or_else(|| crate::StoreError::D1("save_signal failed: no id returned".into()))?;
        for aid in evidence_ids {
            let _ = self.db.prepare("INSERT OR IGNORE INTO signal_evidence (signal_id, article_id) VALUES (?1, ?2)")
                .bind(&[JsValue::from_f64(signal_id as f64), JsValue::from_f64(*aid as f64)])?.run().await;
        }
        for eid in related_ids {
            let _ = self.db.prepare("INSERT OR IGNORE INTO signal_entities (signal_id, entity_id) VALUES (?1, ?2)")
                .bind(&[JsValue::from_f64(signal_id as f64), JsValue::from_f64(*eid as f64)])?.run().await;
        }
        Ok(signal_id)
    }

    /// Load recent intelligence signals.
    pub async fn load_recent_signals(&self, limit: u32, offset: u32) -> Result<Vec<crate::IntelligenceSignal>, crate::StoreError> {
        Ok(self.db.prepare(
            "SELECT id, anchor_entity_id, title, summary, signal_type, confidence, impact, \
                    trend, article_count, source_count, created_at, updated_at \
             FROM intelligence_signals ORDER BY confidence DESC LIMIT ?1 OFFSET ?2",
        ).bind(&[JsValue::from_f64(limit as f64), JsValue::from_f64(offset as f64)])?.all().await?.results()?)
    }

    /// Load a single intelligence signal by id.
    pub async fn load_signal_by_id(&self, id: i64) -> Result<Option<crate::IntelligenceSignal>, crate::StoreError> {
        let r = self.db.prepare(
            "SELECT id, anchor_entity_id, title, summary, signal_type, confidence, impact, \
                    trend, article_count, source_count, created_at, updated_at \
             FROM intelligence_signals WHERE id = ?1",
        ).bind(&[JsValue::from_f64(id as f64)])?.first::<crate::IntelligenceSignal>(None).await?;
        Ok(r)
    }

    /// Load signals anchored to a specific entity.
    pub async fn entity_signals(&self, entity_id: i64, limit: u32) -> Result<Vec<crate::IntelligenceSignal>, crate::StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT id, anchor_entity_id, title, summary, signal_type, confidence, impact, \
                        trend, article_count, source_count, created_at, updated_at \
                 FROM intelligence_signals \
                 WHERE anchor_entity_id = ?1 \
                 ORDER BY created_at DESC \
                 LIMIT ?2",
            )
            .bind(&[JsValue::from_f64(entity_id as f64), JsValue::from_f64(limit as f64)])?
            .all()
            .await?
            .results()?)
    }
}
