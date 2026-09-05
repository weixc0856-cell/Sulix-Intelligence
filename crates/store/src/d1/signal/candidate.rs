use crate::s_err::StoreResultExt;
use serde::Deserialize;
use worker::wasm_bindgen::JsValue;

impl crate::D1Store {
    /// Generate entity-anchored signal candidates with 5-factor scoring.
    #[allow(clippy::too_many_arguments)]
    pub async fn entity_signal_candidates(
        &self,
        now: i64,
        days: i64,
        limit: u32,
    ) -> Result<Vec<crate::EntitySignalCandidate>, crate::StoreError> {
        let cutoff = now - days * 86400;
        let hist_cutoff = now - (days + 21) * 86400;

        #[derive(Deserialize)]
        struct EntityRow {
            entity_id: i64,
            entity_name: String,
            entity_type: String,
            article_count: i64,
            source_count: i64,
            avg_score: f64,
        }

        let rows: Vec<EntityRow> = self
            .db
            .prepare(
                "SELECT ae.entity_id, e.name AS entity_name, e.entity_type, \
                        COUNT(*) AS article_count, \
                        COUNT(DISTINCT a.feed_id) AS source_count, \
                        COALESCE(AVG(a.score), 0) AS avg_score \
                 FROM article_entities ae \
                 JOIN entities e ON e.id = ae.entity_id \
                 JOIN articles a ON a.id = ae.article_id \
                 WHERE a.published_at >= ?1 \
                 GROUP BY ae.entity_id \
                 HAVING article_count >= 2 \
                 ORDER BY article_count DESC \
                 LIMIT ?2",
            )
            .bind(&[JsValue::from_f64(cutoff as f64), JsValue::from_f64(limit as f64)])
            .s_err()?
            .all()
            .await
            .s_err()?
            .results()
            .s_err()?;

        if rows.is_empty() {
            return Ok(Vec::new());
        }

        let mut candidates: Vec<crate::EntitySignalCandidate> = Vec::with_capacity(rows.len());

        for row in &rows {
            let recent_cutoff = now - 3 * 86400;
            let earlier_cutoff = now - 6 * 86400;

            let recent_count: i64 = self
                .db
                .prepare(
                    "SELECT COUNT(*) AS cnt FROM article_entities ae \
                     JOIN articles a ON a.id = ae.article_id \
                     WHERE ae.entity_id = ?1 AND a.published_at >= ?2",
                )
                .bind(&[JsValue::from_f64(row.entity_id as f64), JsValue::from_f64(recent_cutoff as f64)])
                .s_err()?
                .first::<serde_json::Value>(None)
                .await
                .s_err()?
                .and_then(|r| r["cnt"].as_i64())
                .unwrap_or(0);

            let earlier_count: i64 = self
                .db
                .prepare(
                    "SELECT COUNT(*) AS cnt FROM article_entities ae \
                     JOIN articles a ON a.id = ae.article_id \
                     WHERE ae.entity_id = ?1 AND a.published_at >= ?2 AND a.published_at < ?3",
                )
                .bind(&[
                    JsValue::from_f64(row.entity_id as f64),
                    JsValue::from_f64(earlier_cutoff as f64),
                    JsValue::from_f64(recent_cutoff as f64),
                ])
                .s_err()?
                .first::<serde_json::Value>(None)
                .await
                .s_err()?
                .and_then(|r| r["cnt"].as_i64())
                .unwrap_or(0);

            let trend = if earlier_count == 0 || recent_count > earlier_count * 12 / 10 {
                "rising"
            } else if recent_count < earlier_count * 8 / 10 {
                "declining"
            } else {
                "stable"
            };

            // Novelty: current rate vs historical 21d average
            let historical_count: i64 = self
                .db
                .prepare(
                    "SELECT COUNT(*) AS cnt FROM article_entities ae \
                     JOIN articles a ON a.id = ae.article_id \
                     WHERE ae.entity_id = ?1 AND a.published_at >= ?2 AND a.published_at < ?3",
                )
                .bind(&[
                    JsValue::from_f64(row.entity_id as f64),
                    JsValue::from_f64(hist_cutoff as f64),
                    JsValue::from_f64(cutoff as f64),
                ])
                .s_err()?
                .first::<serde_json::Value>(None)
                .await
                .s_err()?
                .and_then(|r| r["cnt"].as_i64())
                .unwrap_or(0);

            let current_rate = row.article_count as f64 / days as f64;
            let historical_rate =
                if historical_count > 0 { historical_count as f64 / 21.0 } else { current_rate * 0.5 };
            let novelty_raw = if historical_rate > 0.0 { current_rate / historical_rate } else { 1.0 };

            // Fixed normalization (cross-day comparable)
            let volume = (row.article_count as f64 / days as f64).min(20.0) / 20.0;
            let diversity = (row.source_count as f64).min(10.0) / 10.0;
            let quality = (row.avg_score / 10.0).clamp(0.0, 1.0);
            let velocity = match trend {
                "rising" => 1.0,
                "stable" => 0.5,
                _ => 0.0,
            };
            let novelty = (novelty_raw / 3.0).min(1.0);
            let score = 0.25 * volume + 0.20 * diversity + 0.20 * quality + 0.20 * velocity + 0.15 * novelty;

            // Evidence for this entity
            #[derive(Deserialize)]
            struct EvRow {
                id: i64,
                title: String,
                url: Option<String>,
                feed_name: Option<String>,
                published_at: Option<i64>,
                score: f64,
            }

            let evidence: Vec<EvRow> = self
                .db
                .prepare(
                    "SELECT a.id, a.title, a.url, f.title AS feed_name, a.published_at, a.score \
                     FROM article_entities ae \
                     JOIN articles a ON a.id = ae.article_id \
                     LEFT JOIN feeds f ON f.id = a.feed_id \
                     WHERE ae.entity_id = ?1 AND a.published_at >= ?2 \
                     ORDER BY a.score DESC LIMIT 10",
                )
                .bind(&[JsValue::from_f64(row.entity_id as f64), JsValue::from_f64(cutoff as f64)])
                .s_err()?
                .all()
                .await
                .s_err()?
                .results()
                .s_err()?;

            let evidence_articles: Vec<crate::SignalEvidence> = evidence
                .into_iter()
                .map(|e| crate::SignalEvidence {
                    id: e.id,
                    title: e.title,
                    url: e.url,
                    feed_name: e.feed_name,
                    published_at: e.published_at,
                    score: e.score,
                })
                .collect();

            candidates.push(crate::EntitySignalCandidate {
                entity_id: row.entity_id,
                entity_name: row.entity_name.clone(),
                entity_type: row.entity_type.clone(),
                score,
                volume,
                diversity,
                quality,
                velocity,
                novelty,
                article_count: row.article_count,
                source_count: row.source_count,
                avg_score: row.avg_score,
                trend: trend.into(),
                evidence: evidence_articles,
                related_entity_ids: Vec::new(),
            });
        }

        candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        Ok(candidates)
    }

    /// Generate entity signal candidates with quality filtering.
    ///
    /// Applies the same 5-factor scoring as [`entity_signal_candidates`] but
    /// adds filters to exclude low-quality entity noise:
    /// - `min_entity_articles` — minimum articles for an entity to qualify.
    /// - `min_sources` — minimum distinct feed sources required.
    /// - Always excludes `entity_type = 'unknown'`.
    #[allow(clippy::too_many_arguments)]
    pub async fn entity_signal_candidates_filtered(
        &self,
        now: i64,
        days: i64,
        limit: u32,
        min_entity_articles: u32,
        min_sources: u32,
    ) -> Result<Vec<crate::EntitySignalCandidate>, crate::StoreError> {
        let cutoff = now - days * 86400;
        let hist_cutoff = now - (days + 21) * 86400;

        #[derive(Deserialize)]
        struct EntityRow {
            entity_id: i64,
            entity_name: String,
            entity_type: String,
            article_count: i64,
            source_count: i64,
            avg_score: f64,
        }

        let rows: Vec<EntityRow> = self
            .db
            .prepare(
                "SELECT ae.entity_id, e.name AS entity_name, e.entity_type, \
                        COUNT(*) AS article_count, \
                        COUNT(DISTINCT a.feed_id) AS source_count, \
                        COALESCE(AVG(a.score), 0) AS avg_score \
                 FROM article_entities ae \
                 JOIN entities e ON e.id = ae.entity_id \
                 JOIN articles a ON a.id = ae.article_id \
                 WHERE a.published_at >= ?1 \
                   AND e.entity_type != 'unknown' \
                 GROUP BY ae.entity_id \
                 HAVING article_count >= ?3 AND source_count >= ?4 \
                 ORDER BY article_count DESC \
                 LIMIT ?2",
            )
            .bind(&[
                JsValue::from_f64(cutoff as f64),
                JsValue::from_f64(limit as f64),
                JsValue::from_f64(min_entity_articles as f64),
                JsValue::from_f64(min_sources as f64),
            ])
            .s_err()?
            .all()
            .await
            .s_err()?
            .results()
            .s_err()?;

        if rows.is_empty() {
            return Ok(Vec::new());
        }

        let mut candidates: Vec<crate::EntitySignalCandidate> = Vec::with_capacity(rows.len());

        for row in &rows {
            let recent_cutoff = now - 3 * 86400;
            let earlier_cutoff = now - 6 * 86400;

            let recent_count: i64 = self
                .db
                .prepare(
                    "SELECT COUNT(*) AS cnt FROM article_entities ae \
                     JOIN articles a ON a.id = ae.article_id \
                     WHERE ae.entity_id = ?1 AND a.published_at >= ?2",
                )
                .bind(&[JsValue::from_f64(row.entity_id as f64), JsValue::from_f64(recent_cutoff as f64)])
                .s_err()?
                .first::<serde_json::Value>(None)
                .await
                .s_err()?
                .and_then(|r| r["cnt"].as_i64())
                .unwrap_or(0);

            let earlier_count: i64 = self
                .db
                .prepare(
                    "SELECT COUNT(*) AS cnt FROM article_entities ae \
                     JOIN articles a ON a.id = ae.article_id \
                     WHERE ae.entity_id = ?1 AND a.published_at >= ?2 AND a.published_at < ?3",
                )
                .bind(&[
                    JsValue::from_f64(row.entity_id as f64),
                    JsValue::from_f64(earlier_cutoff as f64),
                    JsValue::from_f64(recent_cutoff as f64),
                ])
                .s_err()?
                .first::<serde_json::Value>(None)
                .await
                .s_err()?
                .and_then(|r| r["cnt"].as_i64())
                .unwrap_or(0);

            let trend = if earlier_count == 0 || recent_count > earlier_count * 12 / 10 {
                "rising"
            } else if recent_count < earlier_count * 8 / 10 {
                "declining"
            } else {
                "stable"
            };

            let historical_count: i64 = self
                .db
                .prepare(
                    "SELECT COUNT(*) AS cnt FROM article_entities ae \
                     JOIN articles a ON a.id = ae.article_id \
                     WHERE ae.entity_id = ?1 AND a.published_at >= ?2 AND a.published_at < ?3",
                )
                .bind(&[
                    JsValue::from_f64(row.entity_id as f64),
                    JsValue::from_f64(hist_cutoff as f64),
                    JsValue::from_f64(cutoff as f64),
                ])
                .s_err()?
                .first::<serde_json::Value>(None)
                .await
                .s_err()?
                .and_then(|r| r["cnt"].as_i64())
                .unwrap_or(0);

            let current_rate = row.article_count as f64 / days as f64;
            let historical_rate =
                if historical_count > 0 { historical_count as f64 / 21.0 } else { current_rate * 0.5 };
            let novelty_raw = if historical_rate > 0.0 { current_rate / historical_rate } else { 1.0 };

            let volume = (row.article_count as f64 / days as f64).min(20.0) / 20.0;
            let diversity = (row.source_count as f64).min(10.0) / 10.0;
            let quality = (row.avg_score / 10.0).clamp(0.0, 1.0);
            let velocity = match trend {
                "rising" => 1.0,
                "stable" => 0.5,
                _ => 0.0,
            };
            let novelty = (novelty_raw / 3.0).min(1.0);
            let score = 0.25 * volume + 0.20 * diversity + 0.20 * quality + 0.20 * velocity + 0.15 * novelty;

            #[derive(Deserialize)]
            struct EvRow {
                id: i64,
                title: String,
                url: Option<String>,
                feed_name: Option<String>,
                published_at: Option<i64>,
                score: f64,
            }

            let evidence: Vec<EvRow> = self
                .db
                .prepare(
                    "SELECT a.id, a.title, a.url, f.title AS feed_name, a.published_at, a.score \
                     FROM article_entities ae \
                     JOIN articles a ON a.id = ae.article_id \
                     LEFT JOIN feeds f ON f.id = a.feed_id \
                     WHERE ae.entity_id = ?1 AND a.published_at >= ?2 \
                     ORDER BY a.score DESC LIMIT 10",
                )
                .bind(&[JsValue::from_f64(row.entity_id as f64), JsValue::from_f64(cutoff as f64)])
                .s_err()?
                .all()
                .await
                .s_err()?
                .results()
                .s_err()?;

            let evidence_articles: Vec<crate::SignalEvidence> = evidence
                .into_iter()
                .map(|e| crate::SignalEvidence {
                    id: e.id,
                    title: e.title,
                    url: e.url,
                    feed_name: e.feed_name,
                    published_at: e.published_at,
                    score: e.score,
                })
                .collect();

            candidates.push(crate::EntitySignalCandidate {
                entity_id: row.entity_id,
                entity_name: row.entity_name.clone(),
                entity_type: row.entity_type.clone(),
                score,
                volume,
                diversity,
                quality,
                velocity,
                novelty,
                article_count: row.article_count,
                source_count: row.source_count,
                avg_score: row.avg_score,
                trend: trend.into(),
                evidence: evidence_articles,
                related_entity_ids: Vec::new(),
            });
        }

        candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        Ok(candidates)
    }

    /// Build today's intelligence signals using entity-driven ranking.
    pub async fn signals_today(&self, now: i64) -> Result<Vec<crate::TodaySignal>, crate::StoreError> {
        let candidates = self.entity_signal_candidates(now, 7, 30).await?;

        let signals: Vec<crate::TodaySignal> = candidates
            .into_iter()
            .map(|c| {
                let evidence_count = c.evidence.len() as i64;
                let confidence = c.score.min(1.0);
                let summary = format!(
                    "{} — {} articles across {} sources. Score: {:.1}, Trend: {}",
                    c.entity_name, c.article_count, c.source_count, c.avg_score, c.trend
                );
                crate::TodaySignal {
                    id: format!("entity_{}", c.entity_id),
                    title: c.entity_name.clone(),
                    summary,
                    confidence,
                    evidence_count,
                    trend: c.trend.clone(),
                    articles: c.evidence,
                    origin: crate::SignalOrigin::Entity,
                    anchor_entity: Some(crate::EntitySignalRef {
                        id: c.entity_id,
                        name: c.entity_name.clone(),
                        entity_type: c.entity_type.clone(),
                    }),
                }
            })
            .collect();

        Ok(signals)
    }
}
