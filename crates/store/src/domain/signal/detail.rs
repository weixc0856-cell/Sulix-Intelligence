//! Signal Detail — assemble SignalDetail for human investigation.
//!
//! SignalDetail is the "human investigation" model, distinct from
//! SignalBriefInput (the "LLM consumption" model).
//!
//! Methods are on D1Store to compose thread, instances, evidence,
//! entities, and related signals into a single SignalDetail response.

use worker::wasm_bindgen::JsValue;

use crate::{
    BriefArticle, EntitySignalRef, HealthComponents, RelatedEntityRef, RelatedSignalRef, SignalBriefInput,
    SignalDetail, SignalHealthDetail2, SignalTimelineEvent, StoreError,
};

impl crate::D1Store {
    /// Load a single signal thread's detail view.
    pub async fn load_signal_detail(&self, thread_id: i64) -> Result<Option<SignalDetail>, StoreError> {
        // 1. Load thread via existing list method (filter to one thread by id)
        // We can't easily filter by id in list_signal_threads, so query directly
        let (thread, first_seen_opt, _last_opt) = match self.load_single_thread(thread_id).await? {
            Some(result) => result,
            None => return Ok(None),
        };
        let first_seen = first_seen_opt.unwrap_or(0);
        let last_seen = _last_opt.unwrap_or(0);

        let now = (js_sys::Date::now() / 1000.0) as i64;

        // 2. Compute health
        let health = build_health(&thread, now);

        // 3. Build timeline from instances
        let timeline = build_timeline(&thread.instances, first_seen);

        // 4. Load evidence (top 10)
        let evidence = self.load_signal_detail_evidence(thread_id).await?;

        // 5. Load related entities
        let related_entities = self.load_signal_related_entities(thread_id).await?;

        // 6. Load related signals (other threads sharing anchor entity)
        let related_signals = self.load_related_signals(thread_id).await?;

        Ok(Some(SignalDetail {
            id: thread_id,
            title: thread.title,
            description: thread.description,
            status: thread.status,
            trend: thread.trend,
            health,
            anchor_entity: thread.anchor_entity.as_deref().map(|name| EntitySignalRef {
                id: 0,
                name: name.to_string(),
                entity_type: String::new(),
            }),
            first_seen_at: first_seen,
            last_seen_at: last_seen,
            timeline,
            evidence_top: evidence,
            related_entities,
            related_signals,
        }))
    }

    /// Load a single thread by id, reusing assembly logic.
    async fn load_single_thread(
        &self,
        thread_id: i64,
    ) -> Result<Option<(SignalBriefInput, Option<i64>, Option<i64>)>, StoreError> {
        // Query thread + entity
        #[derive(serde::Deserialize)]
        struct ThreadRow {
            id: i64,
            title: String,
            description: String,
            status: String,
            health_score: f64,
            entity_name: Option<String>,
            first_seen_at: Option<i64>,
            last_seen_at: Option<i64>,
        }

        let thread_row: Option<ThreadRow> = self
            .db
            .prepare(
                "SELECT t.id, t.title, t.description, t.status, t.health_score, \
                        e.name AS entity_name, t.first_seen_at, t.last_seen_at \
                 FROM signal_threads t \
                 LEFT JOIN entities e ON e.id = t.anchor_entity_id \
                 WHERE t.id = ?1",
            )
            .bind(&[JsValue::from_f64(thread_id as f64)])?
            .first::<ThreadRow>(None)
            .await?;

        let t = match thread_row {
            Some(t) => t,
            None => return Ok(None),
        };

        // Load instances
        let instances: Vec<crate::SignalInstanceSummary> = self
            .db
            .prepare(
                "SELECT id, score, confidence, trend, article_count, source_count, created_at AS generated_at \
                 FROM intelligence_signals WHERE signal_thread_id = ?1 ORDER BY created_at DESC LIMIT 30",
            )
            .bind(&[JsValue::from_f64(thread_id as f64)])?
            .all()
            .await?
            .results()?;

        // Load evidence
        let ev: Vec<BriefArticle> = self
            .db
            .prepare(
                "SELECT DISTINCT se.article_id AS id, a.title, a.url, f.title AS feed_name, a.score \
                 FROM signal_evidence se JOIN articles a ON a.id = se.article_id \
                 LEFT JOIN feeds f ON f.id = a.feed_id \
                 WHERE se.signal_id IN (SELECT id FROM intelligence_signals WHERE signal_thread_id = ?1) \
                 ORDER BY a.score DESC LIMIT 10",
            )
            .bind(&[JsValue::from_f64(thread_id as f64)])?
            .all()
            .await?
            .results()?;

        let current_score = instances.first().map(|i| i.score).unwrap_or(0.0);
        let trend = instances.first().map(|i| i.trend.clone()).unwrap_or_else(|| "stable".into());
        let recent_count: i64 = instances.iter().map(|i| i.article_count).sum();

        let brief = SignalBriefInput {
            thread_id: t.id,
            signal_key: String::new(),
            anchor_entity: t.entity_name,
            title: t.title,
            description: t.description,
            status: t.status,
            health_score: t.health_score,
            current_score,
            trend,
            cumulative_article_count: recent_count,
            recent_article_count: recent_count,
            source_count: instances.first().map(|i| i.source_count).unwrap_or(0),
            velocity: 0.5,
            instances,
            evidence: ev,
            related_entities: Vec::new(),
        };
        Ok(Some((brief, t.first_seen_at, t.last_seen_at)))
    }

    /// Load top evidence articles for a signal thread.
    async fn load_signal_detail_evidence(&self, thread_id: i64) -> Result<Vec<BriefArticle>, StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT DISTINCT se.article_id AS id, a.title, a.url, f.title AS feed_name, a.score \
                 FROM signal_evidence se JOIN articles a ON a.id = se.article_id \
                 LEFT JOIN feeds f ON f.id = a.feed_id \
                 WHERE se.signal_id IN (SELECT id FROM intelligence_signals WHERE signal_thread_id = ?1) \
                 ORDER BY a.score DESC LIMIT 10",
            )
            .bind(&[JsValue::from_f64(thread_id as f64)])?
            .all()
            .await?
            .results()?)
    }

    /// Load related entities for a signal thread via entity_relations.
    pub async fn load_thread_related_entities(
        &self,
        thread_id: i64,
        limit: u32,
    ) -> Result<Vec<RelatedEntityRef>, StoreError> {
        // Get anchor entity from thread, then get its relations
        Ok(self
            .db
            .prepare(
                "SELECT e.id, e.name, e.entity_type, 'mentioned_together' AS relation_type \
                 FROM signal_threads st \
                 JOIN entity_relations er ON er.source_entity_id = st.anchor_entity_id OR er.target_entity_id = st.anchor_entity_id \
                 JOIN entities e ON e.id = CASE WHEN er.source_entity_id = st.anchor_entity_id THEN er.target_entity_id ELSE er.source_entity_id END \
                 WHERE st.id = ?1 \
                 ORDER BY er.confidence DESC LIMIT ?2",
            )
            .bind(&[JsValue::from_f64(thread_id as f64), JsValue::from_f64(limit as f64)])?
            .all()
            .await?
            .results()?)
    }

    /// Backward-compatible wrapper for internal use.
    async fn load_signal_related_entities(&self, thread_id: i64) -> Result<Vec<RelatedEntityRef>, StoreError> {
        self.load_thread_related_entities(thread_id, 5).await
    }

    /// Load related signals (other threads sharing the same anchor entity).
    async fn load_related_signals(&self, thread_id: i64) -> Result<Vec<RelatedSignalRef>, StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT st.id, st.title, st.status, st.health_score \
                 FROM signal_threads st \
                 WHERE st.anchor_entity_id = (SELECT anchor_entity_id FROM signal_threads WHERE id = ?1) \
                 AND st.id != ?1 AND st.status IN ('active', 'decaying') \
                 ORDER BY st.health_score DESC LIMIT 5",
            )
            .bind(&[JsValue::from_f64(thread_id as f64)])?
            .all()
            .await?
            .results()?)
    }
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/// Build 5-component health from thread data.
pub fn build_health(input: &SignalBriefInput, now: i64) -> SignalHealthDetail2 {
    let instance_count = input.instances.len() as f64;
    let total_days = if instance_count > 1.0 {
        let first = input.instances.last().map(|i| i.generated_at).unwrap_or(now);
        let last = input.instances.first().map(|i| i.generated_at).unwrap_or(now);
        ((last - first) as f64 / 86400.0).max(1.0)
    } else {
        1.0
    };

    let volume = ((input.recent_article_count as f64 / total_days).min(20.0) / 20.0 * 100.0).round() / 100.0;
    let diversity = ((input.source_count as f64).min(15.0) / 15.0 * 100.0).round() / 100.0;
    let quality = ((input.current_score / 10.0).clamp(0.0, 1.0) * 100.0).round() / 100.0;
    let velocity = match input.trend.as_str() {
        "rising" => 1.0,
        "stable" => 0.5,
        _ => 0.15,
    };
    let persistence = (total_days.min(30.0) / 30.0 * 100.0).round() / 100.0;

    let score = 0.25 * volume + 0.20 * diversity + 0.25 * quality + 0.20 * velocity + 0.10 * persistence;

    SignalHealthDetail2 {
        score: (score * 100.0).round() / 100.0,
        components: HealthComponents { volume, diversity, quality, velocity, persistence },
    }
}

/// Build timeline events from signal instances and thread metadata.
pub fn build_timeline(instances: &[crate::SignalInstanceSummary], created_at: i64) -> Vec<SignalTimelineEvent> {
    let mut events: Vec<SignalTimelineEvent> = Vec::new();

    // First event: signal created
    if created_at > 0 {
        events.push(SignalTimelineEvent {
            timestamp: created_at,
            event_type: "created".into(),
            score: 0.0,
            article_count: 0,
            description: "Signal detected and monitoring began".into(),
        });
    }

    // Instance-based events
    for (i, inst) in instances.iter().enumerate() {
        let desc = if i == 0 {
            format!("Current score: {:.2}, trend: {}", inst.score, inst.trend)
        } else if i == instances.len() - 1 {
            format!("Signal detected — score: {:.2}", inst.score)
        } else {
            format!("Updated — score: {:.2}, {} articles", inst.score, inst.article_count)
        };

        events.push(SignalTimelineEvent {
            timestamp: inst.generated_at,
            event_type: if i == instances.len() - 1 { "created".into() } else { "score_changed".into() },
            score: inst.score,
            article_count: inst.article_count,
            description: desc,
        });
    }

    // Sort by timestamp ascending
    events.sort_by_key(|a| a.timestamp);
    events
}
