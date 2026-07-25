//! Domain-specific D1Store impl extracted from lib.rs — Feeds, Tags/Categories/Stats, Rules.
//! Moved from the monolithic impl block in lib.rs to keep the store crate more modular.

use serde::Deserialize;
use worker::wasm_bindgen::JsValue;
use serde_json::Value;
use std::collections::HashSet;

use crate::{D1Store, StoreError, Feed, FeedStats, HealthStats, ScoreDist, DayCount,
            PendingArticle, ArticleDetail, SignalSummary, SignalStrategy, is_cron_healthy,
            in_placeholders};

impl D1Store {
    // ------------------------------------------------------------------
    // Feeds
    // ------------------------------------------------------------------

    /// Feeds due for fetch: active AND past their fetch_interval_sec.
    pub async fn feeds_due_for_fetch(&self, now: i64, category: Option<&str>) -> Result<Vec<Feed>, StoreError> {
        let (sql, _has_cat) = if category.is_some() {
            ("SELECT id, url, title, category, fetch_interval_sec, last_fetched_at, etag, last_modified, status, extraction_level
              FROM feeds WHERE status = 'active' AND category = ?1
              AND (last_fetched_at IS NULL OR ?2 - last_fetched_at >= fetch_interval_sec)", true)
        } else {
            ("SELECT id, url, title, category, fetch_interval_sec, last_fetched_at, etag, last_modified, status, extraction_level
              FROM feeds WHERE status = 'active'
              AND (last_fetched_at IS NULL OR ?1 - last_fetched_at >= fetch_interval_sec)", false)
        };
        let stmt = self.db.prepare(sql);
        let stmt = if let Some(cat) = category {
            stmt.bind(&[cat.into(), JsValue::from_f64(now as f64)])?
        } else {
            stmt.bind(&[JsValue::from_f64(now as f64)])?
        };
        Ok(stmt.all().await?.results()?)
    }

    /// All feeds, regardless of status.  Optional ?status= filter.
    pub async fn all_feeds(&self, status_filter: Option<&str>) -> Result<Vec<Feed>, StoreError> {
        let sql = if status_filter.is_some() {
            "SELECT id, url, title, category, fetch_interval_sec, last_fetched_at, etag, last_modified, status, extraction_level FROM feeds WHERE status = ?1 ORDER BY last_fetched_at DESC"
        } else {
            "SELECT id, url, title, category, fetch_interval_sec, last_fetched_at, etag, last_modified, status, extraction_level FROM feeds ORDER BY last_fetched_at DESC"
        };
        let stmt = self.db.prepare(sql);
        let stmt = if let Some(sf) = status_filter { stmt.bind(&[sf.into()])? } else { stmt };
        Ok(stmt.all().await?.results()?)
    }

    pub async fn get_feed(&self, id: i64) -> Result<Option<Feed>, StoreError> {
        let stmt = self.db.prepare(
            "SELECT id, url, title, category, fetch_interval_sec, last_fetched_at, etag, last_modified, status, extraction_level FROM feeds WHERE id = ?1",
        ).bind(&[JsValue::from_f64(id as f64)])?;
        Ok(stmt.first::<Feed>(None).await?)
    }

    pub async fn insert_feed(
        &self,
        url: &str,
        title: &str,
        category: &str,
        interval: i64,
    ) -> Result<Option<i64>, StoreError> {
        let row = self
            .db
            .prepare("INSERT OR IGNORE INTO feeds (url, title, category, fetch_interval_sec) VALUES (?1, ?2, ?3, ?4) RETURNING id")
            .bind(&[url.into(), title.into(), category.into(), JsValue::from_f64(interval as f64)])?
            .first::<serde_json::Value>(None)
            .await?;
        Ok(row.and_then(|v| v["id"].as_i64()))
    }

    /// Dynamic update: only non-None fields are applied.
    pub async fn update_feed(
        &self,
        id: i64,
        title: Option<&str>,
        category: Option<&str>,
        interval: Option<i64>,
        extraction_level: Option<&str>,
    ) -> Result<(), StoreError> {
        let mut parts: Vec<String> = Vec::new();
        let mut vals: Vec<JsValue> = Vec::new();
        if let Some(v) = title {
            parts.push("title = ?".into());
            vals.push(v.into());
        }
        if let Some(v) = category {
            parts.push("category = ?".into());
            vals.push(v.into());
        }
        if let Some(v) = interval {
            parts.push("fetch_interval_sec = ?".into());
            vals.push(JsValue::from_f64(v as f64));
        }
        if let Some(v) = extraction_level {
            parts.push("extraction_level = ?".into());
            vals.push(v.into());
        }
        if parts.is_empty() {
            return Ok(());
        }
        vals.push(JsValue::from_f64(id as f64));
        self.db.prepare(format!("UPDATE feeds SET {} WHERE id = ?", parts.join(", "))).bind(&vals)?.run().await?;
        Ok(())
    }

    pub async fn set_feed_status(&self, id: i64, status: &str) -> Result<(), StoreError> {
        self.db
            .prepare("UPDATE feeds SET status = ?1 WHERE id = ?2")
            .bind(&[status.into(), JsValue::from_f64(id as f64)])?
            .run()
            .await?;
        Ok(())
    }

    pub async fn record_fetch_result(
        &self,
        feed_id: i64,
        fetched_at: i64,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<(), StoreError> {
        self.db.prepare(
            "UPDATE feeds SET last_fetched_at = ?1, etag = COALESCE(?2, etag), last_modified = COALESCE(?3, last_modified) WHERE id = ?4",
        ).bind(&[
            JsValue::from_f64(fetched_at as f64), etag.map_or(JsValue::null(), |v| v.into()), last_modified.map_or(JsValue::null(), |v| v.into()), JsValue::from_f64(feed_id as f64),
        ])?.run().await?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Tags / Categories / Stats
    // ------------------------------------------------------------------

    pub async fn tags_summary(&self) -> Result<Vec<(String, i64)>, StoreError> {
        #[derive(Deserialize)]
        struct Row {
            ai_tags: String,
        }
        let rows: Vec<Row> = self
            .db
            .prepare("SELECT ai_tags FROM articles WHERE ai_tags IS NOT NULL AND ai_tags != '[]'")
            .all()
            .await?
            .results()?;
        let mut map: std::collections::BTreeMap<String, i64> = std::collections::BTreeMap::new();
        for row in &rows {
            if let Ok(tags) = serde_json::from_str::<Vec<String>>(&row.ai_tags) {
                for tag in tags {
                    *map.entry(tag).or_default() += 1;
                }
            }
        }
        Ok(map.into_iter().collect())
    }

    pub async fn feed_stats(&self) -> Result<Vec<FeedStats>, StoreError> {
        Ok(self.db.prepare(
            "SELECT f.id, f.title, f.url, f.category, f.status, f.last_fetched_at, COUNT(a.id) AS article_count FROM feeds f LEFT JOIN articles a ON a.feed_id = f.id GROUP BY f.id ORDER BY f.last_fetched_at DESC",
        ).all().await?.results()?)
    }

    pub async fn health_stats(&self) -> Result<HealthStats, StoreError> {
        self.db.prepare(
            "SELECT (SELECT COUNT(*) FROM feeds) AS feed_count, (SELECT COUNT(*) FROM feeds WHERE status = 'active') AS active_feed_count, (SELECT COUNT(*) FROM articles) AS article_count, (SELECT MAX(last_fetched_at) FROM feeds) AS last_cron_run_at",
        ).first::<HealthStats>(None).await?.ok_or_else(|| StoreError::D1("health_stats returned no row".into()))
    }

    pub async fn score_distribution(&self) -> Result<ScoreDist, StoreError> {
        Ok(self.db.prepare(
            "SELECT CAST(SUM(CASE WHEN score >= 8 THEN 1 ELSE 0 END) AS INTEGER) AS top, CAST(SUM(CASE WHEN score >= 5 AND score < 8 THEN 1 ELSE 0 END) AS INTEGER) AS medium, CAST(SUM(CASE WHEN score > 0 AND score < 5 THEN 1 ELSE 0 END) AS INTEGER) AS low, CAST(SUM(CASE WHEN score = 0 THEN 1 ELSE 0 END) AS INTEGER) AS unscored FROM articles",
        ).first::<ScoreDist>(None).await?.unwrap_or(ScoreDist { top: 0, medium: 0, low: 0, unscored: 0 }))
    }

    /// Pipeline health metrics for the operations dashboard.
    pub async fn pipeline_status(&self, now: i64) -> Result<serde_json::Value, StoreError> {
        let health = self.health_stats().await?;
        let dist = self.score_distribution().await.unwrap_or(ScoreDist { top: 0, medium: 0, low: 0, unscored: 0 });

        // Feeds that have never been fetched (last_fetched_at IS NULL) or have errors (status != 'active')
        let problem_feeds: i64 = self
            .db
            .prepare("SELECT COUNT(*) AS cnt FROM feeds WHERE status != 'active' OR last_fetched_at IS NULL")
            .first::<serde_json::Value>(None)
            .await?
            .and_then(|v| v["cnt"].as_i64())
            .unwrap_or(0);

        // Articles with AI summaries completed
        let with_summary: i64 = self
            .db
            .prepare("SELECT COUNT(*) AS cnt FROM articles WHERE ai_summary IS NOT NULL AND ai_summary != ''")
            .first::<serde_json::Value>(None)
            .await?
            .and_then(|v| v["cnt"].as_i64())
            .unwrap_or(0);

        // Articles with non-zero scores (affected by strategies)
        let scored: i64 = self
            .db
            .prepare("SELECT COUNT(*) AS cnt FROM articles WHERE score != 0")
            .first::<serde_json::Value>(None)
            .await?
            .and_then(|v| v["cnt"].as_i64())
            .unwrap_or(0);

        // Articles with scores >= 8 (high signal)
        let high_score: i64 = self
            .db
            .prepare("SELECT COUNT(*) AS cnt FROM articles WHERE score >= 8")
            .first::<serde_json::Value>(None)
            .await?
            .and_then(|v| v["cnt"].as_i64())
            .unwrap_or(0);

        // Articles with vector embeddings (vector_id IS NOT NULL)
        let embedded: i64 = self
            .db
            .prepare("SELECT COUNT(*) AS cnt FROM articles WHERE vector_id IS NOT NULL")
            .first::<serde_json::Value>(None)
            .await?
            .and_then(|v| v["cnt"].as_i64())
            .unwrap_or(0);

        Ok(serde_json::json!({
            "cron": {
                "last_run_at": health.last_cron_run_at,
                "healthy": is_cron_healthy(health.last_cron_run_at, now),
            },
            "feeds": {
                "total": health.feed_count,
                "active": health.active_feed_count,
                "problem_feeds": problem_feeds,
            },
            "articles": {
                "total": health.article_count,
                "with_summary": with_summary,
                "scored": scored,
                "high_score": high_score,
                "unscored": dist.unscored,
            },
            "embedding_coverage": {
                "total": health.article_count,
                "embedded": embedded,
                "pending": health.article_count.saturating_sub(embedded),
            },
        }))
    }

    pub async fn article_trend(&self, days: i64) -> Result<Vec<DayCount>, StoreError> {
        Ok(self.db.prepare(
            "SELECT DATE(published_at, 'unixepoch') AS day, COUNT(*) AS cnt FROM articles WHERE published_at IS NOT NULL GROUP BY day ORDER BY day DESC LIMIT ?1",
        ).bind(&[JsValue::from_f64(days as f64)])?.all().await?.results()?)
    }

    /// Get articles that still need AI summarization, oldest first.
    /// Batch size limits per call to stay within Workers CPU time budget.
    pub async fn pending_ai_articles(&self, batch_size: u32) -> Result<Vec<PendingArticle>, StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score, raw_content_r2_key
             FROM articles WHERE (ai_summary IS NULL OR ai_summary = '')
             ORDER BY published_at ASC LIMIT ?1",
            )
            .bind(&[JsValue::from_f64(batch_size as f64)])?
            .all()
            .await?
            .results()?)
    }

    /// Collect R2 keys for articles that match the expiry criteria,
    /// WITHOUT deleting them.  Caller should delete R2 objects after
    /// calling `expire_old_articles`.
    pub async fn expired_article_r2_keys(&self, now: i64, days: i64) -> Result<Vec<String>, StoreError> {
        let cutoff = now - days * 86400;
        let rows: Vec<serde_json::Value> = self
            .db
            .prepare(
                "SELECT raw_content_r2_key FROM articles WHERE published_at < ?1 AND ai_summary != '' AND ai_summary IS NOT NULL AND raw_content_r2_key IS NOT NULL",
            )
            .bind(&[JsValue::from_f64(cutoff as f64)])?
            .all()
            .await?
            .results()?;
        Ok(rows.iter().filter_map(|r| r["raw_content_r2_key"].as_str().map(String::from)).collect())
    }

    /// Delete articles older than `days` whose AI processing is complete.
    /// `now` should be the current unix timestamp (seconds), typically
    /// passed from the caller's js_sys::Date::now().
    /// Protects D1 from unbounded growth as feed volume increases.
    pub async fn expire_old_articles(&self, now: i64, days: i64) -> Result<u64, StoreError> {
        let cutoff = now - days * 86400;
        let stmt = self
            .db
            .prepare("DELETE FROM articles WHERE published_at < ?1 AND ai_summary != '' AND ai_summary IS NOT NULL")
            .bind(&[JsValue::from_f64(cutoff as f64)])?;
        let result = stmt.run().await?;
        Ok(result.meta().ok().flatten().and_then(|m| m.changes).unwrap_or(0) as u64)
    }

    // ------------------------------------------------------------------
    // Rules
    // ------------------------------------------------------------------

    pub async fn active_rule_jsons(&self, audience_tag: &str) -> Result<Vec<String>, StoreError> {
        #[derive(Deserialize)]
        struct Row {
            rule_json: String,
        }
        let rows: Vec<Row> = self
            .db
            .prepare("SELECT rule_json FROM filter_rules WHERE audience_tag = ?1 AND enabled = 1")
            .bind(&[audience_tag.into()])?
            .all()
            .await?
            .results()?;
        Ok(rows.into_iter().map(|r| r.rule_json).collect())
    }

    /// Aggregate strategies by signal_type for the Intelligence dashboard.
    pub async fn signal_summary(&self) -> Result<Vec<SignalSummary>, StoreError> {
        Ok(self.db.prepare(
            "SELECT signal_type, COUNT(*) AS strategy_count, COALESCE(SUM(score_delta), 0) AS total_score_delta,
                    COALESCE(AVG(score_delta), 0) AS avg_score_delta, SUM(CASE WHEN enabled = 1 THEN 1 ELSE 0 END) AS enabled_count
             FROM filter_rules GROUP BY signal_type ORDER BY total_score_delta DESC",
        ).all().await?.results()?)
    }

    pub async fn list_rules(&self) -> Result<Vec<Value>, StoreError> {
        Ok(self.db.prepare(
            "SELECT id, name, signal_type, rule_json, audience_tag, score_delta, enabled, created_at, updated_at FROM filter_rules ORDER BY created_at DESC",
        ).all().await?.results()?)
    }

    pub async fn get_rule(&self, id: i64) -> Result<Option<SignalStrategy>, StoreError> {
        Ok(self.db.prepare(
            "SELECT id, name, signal_type, rule_json, audience_tag, score_delta, enabled, created_at, updated_at FROM filter_rules WHERE id = ?1",
        ).bind(&[JsValue::from_f64(id as f64)])?.first::<SignalStrategy>(None).await?)
    }

    pub async fn insert_rule(
        &self,
        name: &str,
        rule_json: &str,
        audience_tag: &str,
        signal_type: Option<&str>,
        score_delta: f64,
    ) -> Result<Option<i64>, StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self
            .db
            .prepare("INSERT INTO filter_rules (name, rule_json, audience_tag, signal_type, score_delta, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) RETURNING id")
            .bind(&[
            name.into(),
            rule_json.into(),
            audience_tag.into(),
            signal_type.map_or(JsValue::null(), |v| v.into()),
            JsValue::from_f64(score_delta),
            JsValue::from_f64(now as f64),
        ])?
            .first::<serde_json::Value>(None)
            .await?;
        Ok(row.and_then(|v| v["id"].as_i64()))
    }

    pub async fn update_rule(
        &self,
        id: i64,
        name: Option<&str>,
        rule_json: Option<&str>,
        enabled: Option<bool>,
        signal_type: Option<Option<&str>>,
    ) -> Result<(), StoreError> {
        let mut parts: Vec<String> = Vec::new();
        let mut vals: Vec<JsValue> = Vec::new();
        if let Some(v) = name {
            parts.push("name = ?".into());
            vals.push(v.into());
        }
        if let Some(v) = rule_json {
            parts.push("rule_json = ?".into());
            vals.push(v.into());
        }
        if let Some(v) = enabled {
            parts.push("enabled = ?".into());
            vals.push(JsValue::from_f64(if v { 1.0 } else { 0.0 }));
        }
        if let Some(ref v) = signal_type {
            parts.push("signal_type = ?".into());
            vals.push(v.map_or(JsValue::null(), |s| s.into()));
        }
        if parts.is_empty() {
            return Ok(());
        }
        // Always update updated_at when any field changes
        parts.push("updated_at = ?".into());
        vals.push(JsValue::from_f64(js_sys::Date::now() / 1000.0));
        vals.push(JsValue::from_f64(id as f64));
        self.db
            .prepare(format!("UPDATE filter_rules SET {} WHERE id = ?", parts.join(", ")))
            .bind(&vals)?
            .run()
            .await?;
        Ok(())
    }

    pub async fn delete_rule(&self, id: i64) -> Result<(), StoreError> {
        self.db
            .prepare("UPDATE filter_rules SET enabled = 0, updated_at = ?1 WHERE id = ?2")
            .bind(&[JsValue::from_f64(js_sys::Date::now() / 1000.0), JsValue::from_f64(id as f64)])?
            .run()
            .await?;
        Ok(())
    }

    /// Fetch recent articles for preview evaluation, up to `limit` rows.
    /// Joins with feeds to get feed_name.
    pub async fn recent_articles_for_preview(&self, limit: u32) -> Result<Vec<ArticleDetail>, StoreError> {
        Ok(self.db.prepare(
            "SELECT a.id, a.feed_id, f.title AS feed_name, a.guid, a.title, a.url, a.published_at, a.ai_summary, a.ai_tags, a.score
             FROM articles a LEFT JOIN feeds f ON f.id = a.feed_id
             WHERE a.title != ''
             ORDER BY a.published_at DESC LIMIT ?1",
        ).bind(&[JsValue::from_f64(limit as f64)])?.all().await?.results()?)
    }

    /// Check which of the given article IDs still exist in the database.
    /// Used by the R2 garbage collector to identify orphaned objects.
    pub async fn article_ids_exist(&self, ids: &[i64]) -> Result<HashSet<i64>, StoreError> {
        if ids.is_empty() {
            return Ok(HashSet::new());
        }
        let placeholders = in_placeholders(ids.len());
        let sql = format!("SELECT id FROM articles WHERE id IN ({placeholders})");
        let mut stmt = self.db.prepare(&sql);
        let vals: Vec<JsValue> = ids.iter().map(|id| JsValue::from_f64(*id as f64)).collect();
        stmt = stmt.bind(&vals)?;
        let rows: Vec<serde_json::Value> = stmt.all().await?.results()?;
        Ok(rows.iter().filter_map(|r| r["id"].as_i64()).collect())
    }
}
