use worker::wasm_bindgen::JsValue;

impl crate::D1Store {
    /// Upsert an entity by normalized_name. Returns the entity id.
    pub async fn upsert_entity(
        &self,
        name: &str,
        normalized: &str,
        entity_type: &str,
    ) -> Result<i64, crate::StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self
            .db
            .prepare(
                "INSERT OR IGNORE INTO entities (name, normalized_name, entity_type, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5) RETURNING id",
            )
            .bind(&[
                name.into(),
                normalized.into(),
                entity_type.into(),
                JsValue::from_f64(now as f64),
                JsValue::from_f64(now as f64),
            ])?
            .first::<serde_json::Value>(None)
            .await?;

        if let Some(id) = row.and_then(|v| v["id"].as_i64()) {
            return Ok(id);
        }

        // Already exists — update timestamp and return existing id
        let row = self
            .db
            .prepare("UPDATE entities SET updated_at = ?1, entity_type = ?2 WHERE normalized_name = ?3 RETURNING id")
            .bind(&[JsValue::from_f64(now as f64), entity_type.into(), normalized.into()])?
            .first::<serde_json::Value>(None)
            .await?;

        row.and_then(|v| v["id"].as_i64())
            .ok_or_else(|| crate::StoreError::D1("entity upsert failed: no id returned".into()))
    }

    /// Link an article to an entity.
    pub async fn link_article_entity(
        &self,
        article_id: i64,
        entity_id: i64,
        relevance: f64,
        context: Option<&str>,
    ) -> Result<(), crate::StoreError> {
        self.db
            .prepare(
                "INSERT OR IGNORE INTO article_entities (article_id, entity_id, relevance, context) \
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(&[
                JsValue::from_f64(article_id as f64),
                JsValue::from_f64(entity_id as f64),
                JsValue::from_f64(relevance),
                context.map_or(JsValue::null(), |c| c.into()),
            ])?
            .run()
            .await?;
        Ok(())
    }

    /// Link two entities with a directed relation.
    pub async fn link_entity_relation(
        &self,
        source: i64,
        target: i64,
        rtype: &str,
        confidence: f64,
    ) -> Result<(), crate::StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        self.db
            .prepare(
                "INSERT OR IGNORE INTO entity_relations \
                 (source_entity_id, target_entity_id, relation_type, confidence, first_seen_at, last_seen_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .bind(&[
                JsValue::from_f64(source as f64),
                JsValue::from_f64(target as f64),
                rtype.into(),
                JsValue::from_f64(confidence),
                JsValue::from_f64(now as f64),
                JsValue::from_f64(now as f64),
            ])?
            .run()
            .await?;
        Ok(())
    }

    /// List entities, paginated, ordered by article_count DESC.
    pub async fn list_entities(&self, limit: u32, offset: u32) -> Result<Vec<crate::EntitySummary>, crate::StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT e.id, e.name, e.normalized_name, e.entity_type, e.canonical_id, \
                        COUNT(ae.article_id) AS article_count, \
                        COALESCE(MAX(e.updated_at), 0) AS last_seen \
                 FROM entities e \
                 LEFT JOIN article_entities ae ON ae.entity_id = e.id \
                 GROUP BY e.id \
                 ORDER BY article_count DESC, e.name ASC \
                 LIMIT ?1 OFFSET ?2",
            )
            .bind(&[JsValue::from_f64(limit as f64), JsValue::from_f64(offset as f64)])?
            .all()
            .await?
            .results()?)
    }

    /// Get a single entity by id with aggregate article_count.
    pub async fn entity_detail(&self, id: i64) -> Result<Option<crate::EntityDetail>, crate::StoreError> {
        let result = self
            .db
            .prepare(
                "SELECT e.id, e.name, e.normalized_name, e.entity_type, e.canonical_id, \
                        e.description, e.metadata, \
                        COUNT(ae.article_id) AS article_count, \
                        e.created_at, e.updated_at \
                 FROM entities e \
                 LEFT JOIN article_entities ae ON ae.entity_id = e.id \
                 WHERE e.id = ?1 \
                 GROUP BY e.id",
            )
            .bind(&[JsValue::from_f64(id as f64)])?
            .first::<crate::EntityDetail>(None)
            .await?;
        Ok(result)
    }

    /// Get related entities for a given entity.
    pub async fn entity_relations(
        &self,
        entity_id: i64,
        limit: u32,
    ) -> Result<Vec<crate::RelatedEntity>, crate::StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT e.id, e.name, e.entity_type, er.relation_type, er.confidence, er.last_seen_at \
                 FROM entity_relations er \
                 JOIN entities e ON e.id = CASE WHEN er.source_entity_id = ?1 THEN er.target_entity_id ELSE er.source_entity_id END \
                 WHERE er.source_entity_id = ?1 OR er.target_entity_id = ?1 \
                 ORDER BY er.confidence DESC \
                 LIMIT ?2",
            )
            .bind(&[JsValue::from_f64(entity_id as f64), JsValue::from_f64(limit as f64)])?
            .all()
            .await?
            .results()?)
    }

    /// Get entities linked to an article.
    pub async fn article_entities(&self, article_id: i64) -> Result<Vec<crate::EntityRef>, crate::StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT e.id, e.name, e.normalized_name, e.entity_type, ae.relevance, ae.context \
                 FROM entities e \
                 JOIN article_entities ae ON ae.entity_id = e.id \
                 WHERE ae.article_id = ?1 \
                 ORDER BY ae.relevance DESC",
            )
            .bind(&[JsValue::from_f64(article_id as f64)])?
            .all()
            .await?
            .results()?)
    }
}
