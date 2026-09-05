use crate::s_err::StoreResultExt;
use serde::Deserialize;
use worker::wasm_bindgen::JsValue;

use crate::{SignalMutation, SignalUpsertResult};

impl crate::D1Store {
    /// Upsert a signal thread by signal_key.
    /// Sprint 5.10: uses single UPSERT via ON CONFLICT instead of INSERT+UPDATE fallback.
    /// Returns `SignalUpsertResult` indicating whether the thread was created or updated.
    pub async fn upsert_signal_thread(
        &self,
        signal_key: &str,
        anchor_entity_id: Option<i64>,
        title: &str,
        status: &str,
        discovery_method: &crate::DiscoveryMethod,
        discovery_score: Option<f64>,
    ) -> Result<SignalUpsertResult, crate::StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self
            .db
            .prepare(
                "INSERT INTO signal_threads (signal_key, anchor_entity_id, title, status, discovery_method, discovery_score, first_seen_at, last_seen_at, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8, ?8) \
                 ON CONFLICT(signal_key) DO UPDATE SET \
                   title = excluded.title, \
                   last_seen_at = excluded.last_seen_at, \
                   updated_at = excluded.updated_at, \
                   discovery_score = COALESCE(excluded.discovery_score, signal_threads.discovery_score) \
                 RETURNING id, (xmax = 0) AS is_insert",
            )
            .bind(&[
                signal_key.into(),
                anchor_entity_id.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                title.into(),
                status.into(),
                discovery_method.to_string().into(),
                discovery_score.map_or(JsValue::null(), JsValue::from_f64),
                JsValue::from_f64(now as f64),
                JsValue::from_f64(now as f64),
            ]).s_err()?
            .first::<serde_json::Value>(None)
            .await.s_err()?;

        let id = row
            .as_ref()
            .and_then(|v| v["id"].as_i64())
            .ok_or_else(|| crate::StoreError::D1("upsert_signal_thread failed".into()))?;
        let is_insert = row.and_then(|v| v["is_insert"].as_i64()).unwrap_or(0) != 0;
        Ok(SignalUpsertResult {
            id,
            mutation: if is_insert { SignalMutation::Created } else { SignalMutation::Updated },
        })
    }

    /// Append a signal instance to a thread.
    pub async fn append_signal_instance(
        &self,
        thread_id: i64,
        confidence: f64,
        impact: &str,
        trend: &str,
        article_count: i64,
        source_count: i64,
    ) -> Result<i64, crate::StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self
            .db
            .prepare(
                "INSERT INTO intelligence_signals                  (signal_thread_id, anchor_entity_id, title, summary, signal_type, confidence, impact, trend, article_count, source_count, created_at, updated_at)                  VALUES (?1, NULL, '', '', 'entity', ?2, ?3, ?4, ?5, ?6, ?7, ?8) RETURNING id",
            )
            .bind(&[
                JsValue::from_f64(thread_id as f64),
                JsValue::from_f64(confidence),
                impact.into(),
                trend.into(),
                JsValue::from_f64(article_count as f64),
                JsValue::from_f64(source_count as f64),
                JsValue::from_f64(now as f64),
                JsValue::from_f64(now as f64),
            ]).s_err()?
            .first::<serde_json::Value>(None)
            .await.s_err()?;

        row.and_then(|v| v["id"].as_i64()).ok_or_else(|| crate::StoreError::D1("append_signal_instance failed".into()))
    }

    /// Get active signal threads with instances and evidence.
    pub async fn get_active_signal_threads(
        &self,
        limit: u32,
    ) -> Result<Vec<crate::SignalBriefInput>, crate::StoreError> {
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct ThreadRow {
            id: i64,
            signal_key: String,
            anchor_entity_id: Option<i64>,
            title: String,
            description: String,
            status: String,
            health_score: f64,
            entity_name: Option<String>,
        }
        let threads: Vec<ThreadRow> = self
            .db
            .prepare(
                "SELECT t.id, t.signal_key, t.anchor_entity_id, t.title, t.description, t.status, t.health_score,                         e.name AS entity_name                  FROM signal_threads t                  LEFT JOIN entities e ON e.id = t.anchor_entity_id                  WHERE t.status IN ('active', 'decaying')                  ORDER BY t.health_score DESC, t.last_seen_at DESC LIMIT ?1",
            )
            .bind(&[JsValue::from_f64(limit as f64)]).s_err()?
            .all().await.s_err()?.results().s_err()?;

        let mut results = Vec::with_capacity(threads.len());
        for t in &threads {
            let instances: Vec<crate::SignalInstanceSummary> = self
                .db
                .prepare("SELECT id, score, confidence, trend, article_count, source_count, created_at AS generated_at FROM intelligence_signals WHERE signal_thread_id = ?1 ORDER BY created_at DESC LIMIT 30")
                .bind(&[JsValue::from_f64(t.id as f64)]).s_err()?.all().await.s_err()?.results().s_err()?;
            #[derive(Deserialize)]
            struct EvRow {
                article_id: i64,
                title: String,
                url: Option<String>,
                feed_name: Option<String>,
                score: f64,
            }
            let ev: Vec<EvRow> = self
                .db
                .prepare("SELECT DISTINCT se.article_id, a.title, a.url, f.title AS feed_name, a.score FROM signal_evidence se JOIN articles a ON a.id = se.article_id LEFT JOIN feeds f ON f.id = a.feed_id WHERE se.signal_id IN (SELECT id FROM intelligence_signals WHERE signal_thread_id = ?1) ORDER BY a.score DESC LIMIT 10")
                .bind(&[JsValue::from_f64(t.id as f64)]).s_err()?.all().await.s_err()?.results().s_err()?;
            let related = self.load_thread_related_entities(t.id, 5).await?;
            results.push(crate::SignalBriefInput {
                thread_id: t.id,
                signal_key: t.signal_key.clone(),
                anchor_entity: t.entity_name.clone(),
                title: t.title.clone(),
                description: t.description.clone(),
                status: t.status.clone(),
                health_score: t.health_score,
                current_score: instances.first().map(|i| i.score).unwrap_or(0.0),
                trend: instances.first().map(|i| i.trend.clone()).unwrap_or_else(|| "stable".into()),
                cumulative_article_count: instances.iter().map(|i| i.article_count).sum(),
                recent_article_count: instances.iter().map(|i| i.article_count).sum(),
                source_count: instances.first().map(|i| i.source_count).unwrap_or(0),
                velocity: 0.5,
                instances,
                evidence: ev
                    .into_iter()
                    .map(|r| crate::BriefArticle {
                        id: r.article_id,
                        title: r.title,
                        url: r.url,
                        feed_name: r.feed_name,
                        score: r.score,
                    })
                    .collect(),
                related_entities: related,
                provenance: Default::default(),
            });
        }
        Ok(results)
    }

    /// List signal threads with dynamic filtering (statuses, min_score, limit).
    /// Returns fully-populated [`SignalBriefInput`] including derived metrics
    /// computed from the thread's instance history.
    pub async fn list_signal_threads(
        &self,
        filter: &crate::SignalThreadFilter,
    ) -> Result<Vec<crate::SignalBriefInput>, crate::StoreError> {
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct ThreadRow {
            id: i64,
            signal_key: String,
            anchor_entity_id: Option<i64>,
            title: String,
            description: String,
            status: String,
            health_score: f64,
            entity_name: Option<String>,
        }

        let status_count = filter.statuses.len();
        let placeholders = crate::in_placeholders(status_count);
        let mut sql = format!(
            "SELECT t.id, t.signal_key, t.anchor_entity_id, t.title, t.description, t.status, t.health_score, \
                    e.name AS entity_name \
             FROM signal_threads t \
             LEFT JOIN entities e ON e.id = t.anchor_entity_id \
             WHERE t.status IN ({placeholders})"
        );
        let mut binds: Vec<JsValue> = filter.statuses.iter().map(|s| s.as_str().into()).collect();

        if filter.min_score > 0.0 {
            let idx = binds.len() + 1;
            sql.push_str(&format!(" AND t.health_score >= ?{idx}"));
            binds.push(JsValue::from_f64(filter.min_score));
        }

        {
            let idx = binds.len() + 1;
            sql.push_str(&format!(" ORDER BY t.health_score DESC, t.last_seen_at DESC LIMIT ?{idx}"));
            binds.push(JsValue::from_f64(filter.limit as f64));
        }

        let threads: Vec<ThreadRow> =
            self.db.prepare(&sql).bind(&binds).s_err()?.all().await.s_err()?.results().s_err()?;

        let now = (js_sys::Date::now() / 1000.0) as i64;
        let seven_days_ago = now - 7 * 86400;

        let mut results = Vec::with_capacity(threads.len());
        for t in &threads {
            let instances: Vec<crate::SignalInstanceSummary> = self
                .db
                .prepare(
                    "SELECT id, score, confidence, trend, article_count, source_count, created_at AS generated_at \
                     FROM intelligence_signals WHERE signal_thread_id = ?1 ORDER BY created_at DESC LIMIT 30",
                )
                .bind(&[JsValue::from_f64(t.id as f64)])
                .s_err()?
                .all()
                .await
                .s_err()?
                .results()
                .s_err()?;

            #[derive(Deserialize)]
            struct EvRow {
                article_id: i64,
                title: String,
                url: Option<String>,
                feed_name: Option<String>,
                score: f64,
            }
            let ev: Vec<EvRow> = self
                .db
                .prepare(
                    "SELECT DISTINCT se.article_id, a.title, a.url, f.title AS feed_name, a.score \
                     FROM signal_evidence se \
                     JOIN articles a ON a.id = se.article_id \
                     LEFT JOIN feeds f ON f.id = a.feed_id \
                     WHERE se.signal_id IN (SELECT id FROM intelligence_signals WHERE signal_thread_id = ?1) \
                     ORDER BY a.score DESC LIMIT 10",
                )
                .bind(&[JsValue::from_f64(t.id as f64)])
                .s_err()?
                .all()
                .await
                .s_err()?
                .results()
                .s_err()?;

            let current_score = instances.first().map(|i| i.score).unwrap_or(0.0);
            let trend = instances.first().map(|i| i.trend.clone()).unwrap_or_else(|| "stable".into());
            let cumulative_article_count: i64 = instances.iter().map(|i| i.article_count).sum();
            let recent_article_count: i64 =
                instances.iter().filter(|i| i.generated_at >= seven_days_ago).map(|i| i.article_count).sum();
            let source_count = instances.first().map(|i| i.source_count).unwrap_or(0);
            let velocity = if cumulative_article_count > 0 {
                recent_article_count as f64 / cumulative_article_count as f64
            } else {
                0.5
            };

            let related = self.load_thread_related_entities(t.id, 5).await?;
            results.push(crate::SignalBriefInput {
                thread_id: t.id,
                signal_key: t.signal_key.clone(),
                anchor_entity: t.entity_name.clone(),
                title: t.title.clone(),
                description: t.description.clone(),
                status: t.status.clone(),
                health_score: t.health_score,
                current_score,
                trend,
                cumulative_article_count,
                recent_article_count,
                source_count,
                velocity,
                instances,
                evidence: ev
                    .into_iter()
                    .map(|r| crate::BriefArticle {
                        id: r.article_id,
                        title: r.title,
                        url: r.url,
                        feed_name: r.feed_name,
                        score: r.score,
                    })
                    .collect(),
                related_entities: related,
                provenance: Default::default(),
            });
        }
        Ok(results)
    }

    // ── Batch queries (eliminate N+1 for Radar / Projection) ──

    /// Batch-load evidence across multiple threads in a single query.
    /// Returns `HashMap<thread_id, Vec<BriefArticle>>` — each Vec truncated to 10.
    pub async fn batch_evidence(
        &self,
        thread_ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, Vec<crate::BriefArticle>>, crate::StoreError> {
        if thread_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let placeholders = crate::in_placeholders(thread_ids.len());
        let binds: Vec<JsValue> = thread_ids.iter().map(|id| JsValue::from_f64(*id as f64)).collect();

        let sql = format!(
            "SELECT sig.signal_thread_id AS thread_id, \
                    se.article_id AS id, a.title, a.url, f.title AS feed_name, a.score \
             FROM signal_evidence se \
             JOIN articles a ON a.id = se.article_id \
             LEFT JOIN feeds f ON f.id = a.feed_id \
             JOIN intelligence_signals sig ON sig.id = se.signal_id \
             WHERE sig.signal_thread_id IN ({placeholders}) \
             ORDER BY sig.signal_thread_id, a.score DESC"
        );

        #[derive(Deserialize)]
        struct EvBatchRow {
            thread_id: i64,
            id: i64,
            title: String,
            url: Option<String>,
            feed_name: Option<String>,
            score: f64,
        }

        let rows: Vec<EvBatchRow> =
            self.db.prepare(&sql).bind(&binds).s_err()?.all().await.s_err()?.results().s_err()?;
        let mut map: std::collections::HashMap<i64, Vec<crate::BriefArticle>> = std::collections::HashMap::new();
        for row in rows {
            map.entry(row.thread_id).or_default().push(crate::BriefArticle {
                id: row.id,
                title: row.title,
                url: row.url,
                feed_name: row.feed_name,
                score: row.score,
            });
        }
        // Per-thread cap: match the original LIMIT 10 behaviour
        for articles in map.values_mut() {
            articles.truncate(10);
        }
        Ok(map)
    }

    /// Batch-load related entities across multiple threads in a single query.
    /// Returns `HashMap<thread_id, Vec<RelatedEntityRef>>` — each Vec truncated to 5.
    pub async fn batch_related_entities(
        &self,
        thread_ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, Vec<crate::RelatedEntityRef>>, crate::StoreError> {
        if thread_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let placeholders = crate::in_placeholders(thread_ids.len());
        let binds: Vec<JsValue> = thread_ids.iter().map(|id| JsValue::from_f64(*id as f64)).collect();

        let sql = format!(
            "SELECT st.id AS thread_id, e.id, e.name, e.entity_type, 'mentioned_together' AS relation_type \
             FROM signal_threads st \
             JOIN entity_relations er ON er.source_entity_id = st.anchor_entity_id \
                OR er.target_entity_id = st.anchor_entity_id \
             JOIN entities e ON e.id = CASE \
                WHEN er.source_entity_id = st.anchor_entity_id THEN er.target_entity_id \
                ELSE er.source_entity_id \
             END \
             WHERE st.id IN ({placeholders}) \
             ORDER BY st.id, er.confidence DESC"
        );

        #[derive(Deserialize)]
        struct RelBatchRow {
            thread_id: i64,
            id: i64,
            name: String,
            entity_type: String,
            relation_type: String,
        }

        let rows: Vec<RelBatchRow> =
            self.db.prepare(&sql).bind(&binds).s_err()?.all().await.s_err()?.results().s_err()?;
        let mut map: std::collections::HashMap<i64, Vec<crate::RelatedEntityRef>> = std::collections::HashMap::new();
        for row in rows {
            map.entry(row.thread_id).or_default().push(crate::RelatedEntityRef {
                id: row.id,
                name: row.name,
                entity_type: row.entity_type,
                relation_type: row.relation_type,
                relation: None,
                confidence: None,
            });
        }
        // Per-thread cap: match the original LIMIT 5 behaviour
        for entities in map.values_mut() {
            entities.truncate(5);
        }
        Ok(map)
    }
}
