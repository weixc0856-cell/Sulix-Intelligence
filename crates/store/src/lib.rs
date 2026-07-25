//! D1 access layer.  Every other crate (rules, ai-pipeline, search, api)
//! talks to storage only through this crate, so backend swaps never leak
//! into business logic.
//!
//! Type definitions live in [`models`] and are re-exported from the crate
//! root so callers write `store::Feed` / `store::StoreError` etc.

mod models;
pub use models::*;

pub mod backend;
pub mod memory;
pub use backend::StoreBackend;

use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashSet;
use serde_json::Value;
use worker::wasm_bindgen::JsValue;
use worker::D1Database;

/// Production D1-backed store.
pub struct D1Store {
    db: D1Database,
}

/// Backward-compatible alias.
pub type Store = D1Store;

// ---- Pure helper functions (extracted for testability) ----

/// Generate `?1,?2,?3` placeholders for SQL `IN` clauses.
pub(crate) fn in_placeholders(count: usize) -> String {
    (1..=count).map(|i| format!("?{i}")).collect::<Vec<_>>().join(",")
}

/// Build a SQL LIKE pattern that matches a JSON-stringified tag: `%"tag"%`.
pub(crate) fn tag_like_pattern(tag: &str) -> String {
    format!("%\"{}\"%", tag)
}

/// Check whether a cron last-run timestamp is within 3600 seconds of `now`.
pub(crate) fn is_cron_healthy(last_run_at: Option<i64>, now: i64) -> bool {
    last_run_at.is_some_and(|ts| now - ts < 3600)
}

impl D1Store {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }

    // ------------------------------------------------------------------
    // Feeds
    // ------------------------------------------------------------------

    /// Feeds due for fetch: active AND past their fetch_interval_sec.
    pub async fn feeds_due_for_fetch(&self, now: i64, category: Option<&str>) -> Result<Vec<Feed>, StoreError> {
        let (sql, _has_cat) = if category.is_some() {
            // has_cat used below for bind count

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
    // Articles
    // ------------------------------------------------------------------

    pub async fn insert_article(&self, article: &NewArticle) -> Result<Option<i64>, StoreError> {
        let row = self
            .db
            .prepare("INSERT OR IGNORE INTO articles (feed_id, guid, title, url, published_at, raw_content_r2_key) VALUES (?1, ?2, ?3, ?4, ?5, ?6) RETURNING id")
            .bind(&[
            JsValue::from_f64(article.feed_id as f64),
            article.guid.clone().into(),
            article.title.clone().into(),
            article.url.clone().map_or(JsValue::null(), |v| v.into()),
            article.published_at.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
            article.raw_content_r2_key.clone().map_or(JsValue::null(), |v| v.into()),
        ])?
            .first::<serde_json::Value>(None)
            .await?;
        Ok(row.and_then(|v| v["id"].as_i64()))
    }

    pub async fn set_ai_summary(
        &self,
        article_id: i64,
        summary: &str,
        tags_json: &str,
        vector_id: &str,
        score: f64,
    ) -> Result<(), StoreError> {
        self.db
            .prepare("UPDATE articles SET ai_summary = ?1, ai_tags = ?2, vector_id = ?3, score = ?4 WHERE id = ?5")
            .bind(&[
                summary.into(),
                tags_json.into(),
                vector_id.into(),
                JsValue::from_f64(score),
                JsValue::from_f64(article_id as f64),
            ])?
            .run()
            .await?;
        Ok(())
    }

    pub async fn get_raw_content_key(&self, article_id: i64) -> Result<Option<String>, StoreError> {
        #[derive(Deserialize)]
        struct Row {
            raw_content_r2_key: Option<String>,
        }
        Ok(self
            .db
            .prepare("SELECT raw_content_r2_key FROM articles WHERE id = ?1")
            .bind(&[JsValue::from_f64(article_id as f64)])?
            .first::<Row>(None)
            .await?
            .and_then(|r| r.raw_content_r2_key))
    }

    pub async fn set_raw_content_r2_key(&self, article_id: i64, r2_key: Option<&str>) -> Result<(), StoreError> {
        self.db
            .prepare("UPDATE articles SET raw_content_r2_key = ?1 WHERE id = ?2")
            .bind(&[r2_key.into(), JsValue::from_f64(article_id as f64)])?
            .run()
            .await?;
        Ok(())
    }

    pub async fn latest_articles(&self, limit: u32, offset: u32) -> Result<Vec<PendingArticle>, StoreError> {
        Ok(self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles ORDER BY published_at DESC LIMIT ?1 OFFSET ?2",
        ).bind(&[JsValue::from_f64(limit as f64), JsValue::from_f64(offset as f64)])?.all().await?.results()?)
    }

    pub async fn article_count(&self) -> Result<i64, StoreError> {
        let row = self.db.prepare("SELECT COUNT(*) AS cnt FROM articles").first::<serde_json::Value>(None).await?;
        Ok(row.and_then(|v| v["cnt"].as_i64()).unwrap_or(0))
    }

    pub async fn trending_articles(&self, limit: u32, offset: u32) -> Result<Vec<PendingArticle>, StoreError> {
        Ok(self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles WHERE score != 0 ORDER BY score DESC, published_at DESC LIMIT ?1 OFFSET ?2",
        ).bind(&[JsValue::from_f64(limit as f64), JsValue::from_f64(offset as f64)])?.all().await?.results()?)
    }

    pub async fn trending_count(&self) -> Result<i64, StoreError> {
        let row = self
            .db
            .prepare("SELECT COUNT(*) AS cnt FROM articles WHERE score != 0")
            .first::<serde_json::Value>(None)
            .await?;
        Ok(row.and_then(|v| v["cnt"].as_i64()).unwrap_or(0))
    }

    pub async fn article_by_id(&self, id: i64) -> Result<Option<Article>, StoreError> {
        Ok(self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles WHERE id = ?1",
        ).bind(&[JsValue::from_f64(id as f64)])?.first::<Article>(None).await?)
    }

    /// Batch fetch by IDs.  Used by the bookmarks / batch endpoint.
    pub async fn articles_by_ids(&self, ids: &[i64]) -> Result<Vec<Article>, StoreError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let sql = format!(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles WHERE id IN ({})",
            in_placeholders(ids.len())
        );
        let binds: Vec<JsValue> = ids.iter().map(|id| JsValue::from_f64(*id as f64)).collect();
        Ok(self.db.prepare(&sql).bind(&binds)?.all().await?.results()?)
    }

    /// Article with feed metadata joined in for the detail page.
    pub async fn article_detail(&self, id: i64) -> Result<Option<ArticleDetail>, StoreError> {
        Ok(self.db.prepare(
            "SELECT a.id, a.feed_id, f.title AS feed_name, a.guid, a.title, a.url, a.published_at, a.ai_summary, a.ai_tags, a.score
             FROM articles a LEFT JOIN feeds f ON f.id = a.feed_id WHERE a.id = ?1",
        ).bind(&[JsValue::from_f64(id as f64)])?.first::<ArticleDetail>(None).await?)
    }

    /// Get previous and next article relative to a given article id,
    /// ordered by published_at DESC.  Returns (prev, next) �� both may be None.
    pub async fn adjacent_articles(&self, id: i64) -> Result<(Option<Article>, Option<Article>), StoreError> {
        let prev = self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles WHERE published_at < (SELECT COALESCE(published_at, 0) FROM articles WHERE id = ?1) ORDER BY published_at DESC LIMIT 1"
        ).bind(&[JsValue::from_f64(id as f64)])?.first::<Article>(None).await?;
        let next = self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles WHERE published_at > (SELECT COALESCE(published_at, 0) FROM articles WHERE id = ?1) ORDER BY published_at ASC LIMIT 1"
        ).bind(&[JsValue::from_f64(id as f64)])?.first::<Article>(None).await?;
        Ok((prev, next))
    }

    pub async fn articles_by_tag(&self, tag: &str, limit: u32, offset: u32) -> Result<Vec<PendingArticle>, StoreError> {
        let pattern = tag_like_pattern(tag);
        Ok(self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles WHERE ai_tags LIKE ?1 ORDER BY published_at DESC LIMIT ?2 OFFSET ?3",
        ).bind(&[pattern.into(), JsValue::from_f64(limit as f64), JsValue::from_f64(offset as f64)])?.all().await?.results()?)
    }

    pub async fn articles_by_category(
        &self,
        category: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<PendingArticle>, StoreError> {
        Ok(self.db.prepare(
            "SELECT a.id, a.feed_id, a.guid, a.title, a.url, a.published_at, a.ai_summary, a.ai_tags, a.score FROM articles a JOIN feeds f ON f.id = a.feed_id WHERE f.category = ?1 ORDER BY a.published_at DESC LIMIT ?2 OFFSET ?3",
        ).bind(&[category.into(), JsValue::from_f64(limit as f64), JsValue::from_f64(offset as f64)])?.all().await?.results()?)
    }

    pub async fn categories_summary(&self) -> Result<Vec<(String, i64)>, StoreError> {
        #[derive(Deserialize)]
        struct Row {
            category: String,
            article_count: i64,
        }
        let rows: Vec<Row> = self.db.prepare(
            "SELECT f.category, COUNT(a.id) AS article_count FROM feeds f LEFT JOIN articles a ON a.feed_id = f.id WHERE f.category IS NOT NULL AND f.category != '' GROUP BY f.category ORDER BY article_count DESC",
        ).all().await?.results()?;
        Ok(rows.into_iter().map(|r| (r.category, r.article_count)).collect())
    }

    /// Find articles sharing tags with a given article, ordered by match
    /// count desc then recency.  Returns empty when source has no tags.
    pub async fn related_articles(&self, article_id: i64, limit: u32) -> Result<Vec<PendingArticle>, StoreError> {
        let src = self
            .db
            .prepare("SELECT ai_tags FROM articles WHERE id = ?1")
            .bind(&[JsValue::from_f64(article_id as f64)])?;
        let tags_json = match src.first::<String>(None).await? {
            Some(t) => t,
            None => return Ok(Vec::new()),
        };
        let tags: Vec<String> = match serde_json::from_str(&tags_json) {
            Ok(t) => t,
            Err(_) => return Ok(Vec::new()),
        };
        if tags.is_empty() {
            return Ok(Vec::new());
        }
        let conds: Vec<String> = tags.iter().map(|t| format!("ai_tags LIKE '%\"{}%'", t.replace('\'', "''"))).collect();
        let sql = format!(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles WHERE id != ?1 AND ({}) ORDER BY ({} DESC), published_at DESC LIMIT ?2",
            conds.join(" OR "),
            conds.iter().map(|c| format!("CASE WHEN {} THEN 1 ELSE 0 END", c)).collect::<Vec<_>>().join(" + "),
        );
        Ok(self
            .db
            .prepare(&sql)
            .bind(&[JsValue::from_f64(article_id as f64), JsValue::from_f64(limit as f64)])?
            .all()
            .await?
            .results()?)
    }

    // ------------------------------------------------------------------
    // Aggregations
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

    /// Persist a generated daily briefing. Uses INSERT OR REPLACE so
    /// re-generating the same date overwrites the previous version.
    pub async fn save_briefing(
        &self,
        date: &str,
        generated_at: i64,
        signal_count: u32,
        content: &str,
    ) -> Result<(), StoreError> {
        self.db
            .prepare(
                "INSERT OR REPLACE INTO intelligence_briefs (date, generated_at, signal_count, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(&[
                date.into(),
                JsValue::from_f64(generated_at as f64),
                JsValue::from_f64(signal_count as f64),
                content.into(),
                JsValue::from_f64(generated_at as f64),
            ])?
            .run()
            .await?;
        Ok(())
    }

    /// Load today's briefing (the one whose `date` column matches today's
    /// YYYY-MM-DD string). Returns `None` if no briefing was generated yet.
    pub async fn load_today_briefing(&self, date: &str) -> Result<Option<String>, StoreError> {
        let row: Option<serde_json::Value> = self
            .db
            .prepare("SELECT content FROM intelligence_briefs WHERE date = ?1")
            .bind(&[date.into()])?
            .first::<serde_json::Value>(None)
            .await?;
        Ok(row.and_then(|v| v["content"].as_str().map(String::from)))
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

    // ------------------------------------------------------------------
    // Entity-driven Signal Engine (V1.5)
    // ------------------------------------------------------------------

    /// Generate entity-anchored signal candidates with 5-factor scoring.
    #[allow(clippy::too_many_arguments)]
    pub async fn entity_signal_candidates(
        &self,
        now: i64,
        days: i64,
        limit: u32,
    ) -> Result<Vec<EntitySignalCandidate>, StoreError> {
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
            .bind(&[JsValue::from_f64(cutoff as f64), JsValue::from_f64(limit as f64)])?
            .all()
            .await?
            .results()?;

        if rows.is_empty() {
            return Ok(Vec::new());
        }

        let mut candidates: Vec<EntitySignalCandidate> = Vec::with_capacity(rows.len());

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
                .bind(&[JsValue::from_f64(row.entity_id as f64), JsValue::from_f64(recent_cutoff as f64)])?
                .first::<serde_json::Value>(None)
                .await?
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
                ])?
                .first::<serde_json::Value>(None)
                .await?
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
                ])?
                .first::<serde_json::Value>(None)
                .await?
                .and_then(|r| r["cnt"].as_i64())
                .unwrap_or(0);

            let current_rate = row.article_count as f64 / days as f64;
            let historical_rate = if historical_count > 0 {
                historical_count as f64 / 21.0
            } else {
                current_rate * 0.5
            };
            let novelty_raw = if historical_rate > 0.0 { current_rate / historical_rate } else { 1.0 };

            // Fixed normalization (cross-day comparable)
            let volume = (row.article_count as f64 / days as f64).min(20.0) / 20.0;
            let diversity = (row.source_count as f64).min(10.0) / 10.0;
            let quality = (row.avg_score / 10.0).clamp(0.0, 1.0);
            let velocity = match trend { "rising" => 1.0, "stable" => 0.5, _ => 0.0 };
            let novelty = (novelty_raw / 3.0).min(1.0);
            let score = 0.25 * volume + 0.20 * diversity + 0.20 * quality + 0.20 * velocity + 0.15 * novelty;

            // Evidence for this entity
            #[derive(Deserialize)]
            struct EvRow { id: i64, title: String, url: Option<String>, feed_name: Option<String>, published_at: Option<i64>, score: f64 }

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
                .bind(&[JsValue::from_f64(row.entity_id as f64), JsValue::from_f64(cutoff as f64)])?
                .all()
                .await?
                .results()?;

            let evidence_articles: Vec<SignalEvidence> = evidence
                .into_iter()
                .map(|e| SignalEvidence { id: e.id, title: e.title, url: e.url, feed_name: e.feed_name, published_at: e.published_at, score: e.score })
                .collect();

            candidates.push(EntitySignalCandidate {
                entity_id: row.entity_id,
                entity_name: row.entity_name.clone(),
                entity_type: row.entity_type.clone(),
                score, volume, diversity, quality, velocity, novelty,
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
    pub async fn signals_today(&self, now: i64) -> Result<Vec<TodaySignal>, StoreError> {
        let candidates = self.entity_signal_candidates(now, 7, 30).await?;

        let signals: Vec<TodaySignal> = candidates
            .into_iter()
            .map(|c| {
                let evidence_count = c.evidence.len() as i64;
                let confidence = c.score.min(1.0);
                let summary = format!(
                    "{} — {} articles across {} sources. Score: {:.1}, Trend: {}",
                    c.entity_name, c.article_count, c.source_count, c.avg_score, c.trend
                );
                TodaySignal {
                    id: format!("entity_{}", c.entity_id),
                    title: c.entity_name.clone(),
                    summary,
                    confidence,
                    evidence_count,
                    trend: c.trend.clone(),
                    articles: c.evidence,
                    origin: SignalOrigin::Entity,
                    anchor_entity: Some(EntitySignalRef {
                        id: c.entity_id,
                        name: c.entity_name.clone(),
                        entity_type: c.entity_type.clone(),
                    }),
                }
            })
            .collect();

        Ok(signals)
    }

    // ------------------------------------------------------------------
    // Signal Persistence
    // ------------------------------------------------------------------

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
    ) -> Result<i64, StoreError> {
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
            .ok_or_else(|| StoreError::D1("save_signal failed: no id returned".into()))?;
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
    pub async fn load_recent_signals(&self, limit: u32, offset: u32) -> Result<Vec<IntelligenceSignal>, StoreError> {
        Ok(self.db.prepare(
            "SELECT id, anchor_entity_id, title, summary, signal_type, confidence, impact, \
                    trend, article_count, source_count, created_at, updated_at \
             FROM intelligence_signals ORDER BY confidence DESC LIMIT ?1 OFFSET ?2",
        ).bind(&[JsValue::from_f64(limit as f64), JsValue::from_f64(offset as f64)])?.all().await?.results()?)
    }

    /// Load a single intelligence signal by id.
    pub async fn load_signal_by_id(&self, id: i64) -> Result<Option<IntelligenceSignal>, StoreError> {
        let r = self.db.prepare(
            "SELECT id, anchor_entity_id, title, summary, signal_type, confidence, impact, \
                    trend, article_count, source_count, created_at, updated_at \
             FROM intelligence_signals WHERE id = ?1",
        ).bind(&[JsValue::from_f64(id as f64)])?.first::<IntelligenceSignal>(None).await?;
        Ok(r)
    }

    /// Load signals anchored to a specific entity.
    pub async fn entity_signals(&self, entity_id: i64, limit: u32) -> Result<Vec<IntelligenceSignal>, StoreError> {
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



    // ------------------------------------------------------------------
    // Signal Threads
    // ------------------------------------------------------------------

    /// Upsert a signal thread by signal_key. Returns thread id.
    pub async fn upsert_signal_thread(
        &self,
        signal_key: &str,
        anchor_entity_id: Option<i64>,
        title: &str,
        status: &str,
    ) -> Result<i64, StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self
            .db
            .prepare(
                "INSERT INTO signal_threads (signal_key, anchor_entity_id, title, status, first_seen_at, last_seen_at, created_at, updated_at)                  VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, ?6) RETURNING id",
            )
            .bind(&[
                signal_key.into(),
                anchor_entity_id.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                title.into(),
                status.into(),
                JsValue::from_f64(now as f64),
                JsValue::from_f64(now as f64),
            ])?
            .first::<serde_json::Value>(None)
            .await?;

        if let Some(id) = row.and_then(|v| v["id"].as_i64()) {
            return Ok(id);
        }

        let row = self
            .db
            .prepare("UPDATE signal_threads SET title = ?1, updated_at = ?2, last_seen_at = ?3 WHERE signal_key = ?4 RETURNING id")
            .bind(&[title.into(), JsValue::from_f64(now as f64), JsValue::from_f64(now as f64), signal_key.into()])?
            .first::<serde_json::Value>(None)
            .await?;

        row.and_then(|v| v["id"].as_i64())
            .ok_or_else(|| StoreError::D1("upsert_signal_thread failed".into()))
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
    ) -> Result<i64, StoreError> {
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
            ])?
            .first::<serde_json::Value>(None)
            .await?;

        row.and_then(|v| v["id"].as_i64())
            .ok_or_else(|| StoreError::D1("append_signal_instance failed".into()))
    }

    /// Evaluate lifecycle transitions for all active/decaying threads.
    pub async fn update_signal_lifecycle(&self, now: i64) -> Result<(), StoreError> {
        self.db
            .prepare("UPDATE signal_threads SET status = 'decaying', updated_at = ?1 WHERE status = 'active' AND last_seen_at < ?2")
            .bind(&[JsValue::from_f64(now as f64), JsValue::from_f64((now - 7 * 86400) as f64)])?
            .run().await?;
        self.db
            .prepare("UPDATE signal_threads SET status = 'resolved', updated_at = ?1 WHERE status = 'decaying' AND last_seen_at < ?2")
            .bind(&[JsValue::from_f64(now as f64), JsValue::from_f64((now - 14 * 86400) as f64)])?
            .run().await?;
        self.db
            .prepare("UPDATE signal_threads SET status = 'active', updated_at = ?1 WHERE status = 'decaying' AND last_seen_at >= ?2")
            .bind(&[JsValue::from_f64(now as f64), JsValue::from_f64((now - 3 * 86400) as f64)])?
            .run().await?;
        self.db
            .prepare("UPDATE signal_threads SET status = 'archived', updated_at = ?1 WHERE status = 'resolved' AND last_seen_at < ?2")
            .bind(&[JsValue::from_f64(now as f64), JsValue::from_f64((now - 30 * 86400) as f64)])?
            .run().await?;
        Ok(())
    }

    /// Get active signal threads with instances and evidence.
    pub async fn get_active_signal_threads(&self, limit: u32) -> Result<Vec<SignalBriefInput>, StoreError> {
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
            .bind(&[JsValue::from_f64(limit as f64)])?
            .all().await?.results()?;

        let mut results = Vec::with_capacity(threads.len());
        for t in &threads {
            let instances: Vec<SignalInstanceSummary> = self
                .db
                .prepare("SELECT id, score, confidence, trend, article_count, source_count, created_at AS generated_at FROM intelligence_signals WHERE signal_thread_id = ?1 ORDER BY created_at DESC LIMIT 30")
                .bind(&[JsValue::from_f64(t.id as f64)])?.all().await?.results()?;
            #[derive(Deserialize)]
            struct EvRow { article_id: i64, title: String, score: f64 }
            let ev: Vec<EvRow> = self
                .db
                .prepare("SELECT DISTINCT se.article_id, a.title, a.score FROM signal_evidence se JOIN articles a ON a.id = se.article_id WHERE se.signal_id IN (SELECT id FROM intelligence_signals WHERE signal_thread_id = ?1) ORDER BY a.score DESC LIMIT 10")
                .bind(&[JsValue::from_f64(t.id as f64)])?.all().await?.results()?;
            results.push(SignalBriefInput {
                thread_id: t.id,
                anchor_entity: t.entity_name.clone(),
                title: t.title.clone(),
                description: t.description.clone(),
                status: t.status.clone(),
                health_score: t.health_score,
                instances,
                evidence: ev.into_iter().map(|r| BriefArticle { id: r.article_id, title: r.title, score: r.score }).collect(),
                related_entities: Vec::new(),
            });
        }
        Ok(results)
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

    // ------------------------------------------------------------------
    // Entities
    // ------------------------------------------------------------------

    /// Upsert an entity by normalized_name. Returns the entity id.
    pub async fn upsert_entity(&self, name: &str, normalized: &str, entity_type: &str) -> Result<i64, StoreError> {
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
            .ok_or_else(|| StoreError::D1("entity upsert failed: no id returned".into()))
    }

    /// Link an article to an entity.
    pub async fn link_article_entity(
        &self,
        article_id: i64,
        entity_id: i64,
        relevance: f64,
        context: Option<&str>,
    ) -> Result<(), StoreError> {
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
    ) -> Result<(), StoreError> {
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
    pub async fn list_entities(&self, limit: u32, offset: u32) -> Result<Vec<EntitySummary>, StoreError> {
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
    pub async fn entity_detail(&self, id: i64) -> Result<Option<EntityDetail>, StoreError> {
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
            .first::<EntityDetail>(None)
            .await?;
        Ok(result)
    }

    /// Get related entities for a given entity.
    pub async fn entity_relations(&self, entity_id: i64, limit: u32) -> Result<Vec<RelatedEntity>, StoreError> {
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
    pub async fn article_entities(&self, article_id: i64) -> Result<Vec<EntityRef>, StoreError> {
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

    // ------------------------------------------------------------------
    // Entity Intelligence
    // ------------------------------------------------------------------

    /// List articles linked to an entity (Evidence timeline).
    pub async fn entity_articles(
        &self,
        entity_id: i64,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<EntityArticle>, StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT a.id, a.title, a.url, a.published_at, a.ai_summary, a.score, f.title AS feed_name \
                 FROM article_entities ae \
                 JOIN articles a ON a.id = ae.article_id \
                 LEFT JOIN feeds f ON f.id = a.feed_id \
                 WHERE ae.entity_id = ?1 \
                 ORDER BY a.published_at DESC \
                 LIMIT ?2 OFFSET ?3",
            )
            .bind(&[
                JsValue::from_f64(entity_id as f64),
                JsValue::from_f64(limit as f64),
                JsValue::from_f64(offset as f64),
            ])?
            .all()
            .await?
            .results()?)
    }

    /// Activity summary for an entity over the last N days.
    pub async fn entity_activity_summary(
        &self,
        entity_id: i64,
        now: i64,
        days: i64,
    ) -> Result<EntityActivitySummary, StoreError> {
        let cutoff = now - days * 86400;
        let row: Option<serde_json::Value> = self
            .db
            .prepare(
                "SELECT COUNT(*) AS article_count, \
                        COUNT(DISTINCT a.feed_id) AS source_count, \
                        COALESCE(AVG(a.score), 0) AS avg_score, \
                        COALESCE(MAX(a.score), 0) AS max_score, \
                        MIN(a.published_at) AS first_seen_at, \
                        MAX(a.published_at) AS last_seen_at \
                 FROM article_entities ae \
                 JOIN articles a ON a.id = ae.article_id \
                 WHERE ae.entity_id = ?1 AND a.published_at >= ?2",
            )
            .bind(&[JsValue::from_f64(entity_id as f64), JsValue::from_f64(cutoff as f64)])?
            .first::<serde_json::Value>(None)
            .await?;

        let v = row.unwrap_or_default();
        let article_count = v["article_count"].as_i64().unwrap_or(0);
        let source_count = v["source_count"].as_i64().unwrap_or(0);
        let avg_score = v["avg_score"].as_f64().unwrap_or(0.0);
        let max_score = v["max_score"].as_f64().unwrap_or(0.0);
        let first_seen_at = v["first_seen_at"].as_i64();
        let last_seen_at = v["last_seen_at"].as_i64();

        // Compute trend: compare articles in last 3 days vs 3-6 days ago
        let recent_cutoff = now - 3 * 86400;
        let earlier_cutoff = now - 6 * 86400;
        let recent: i64 = self
            .db
            .prepare(
                "SELECT COUNT(*) AS cnt FROM article_entities ae \
                 JOIN articles a ON a.id = ae.article_id \
                 WHERE ae.entity_id = ?1 AND a.published_at >= ?2",
            )
            .bind(&[JsValue::from_f64(entity_id as f64), JsValue::from_f64(recent_cutoff as f64)])?
            .first::<serde_json::Value>(None)
            .await?
            .and_then(|r| r["cnt"].as_i64())
            .unwrap_or(0);
        let earlier: i64 = self
            .db
            .prepare(
                "SELECT COUNT(*) AS cnt FROM article_entities ae \
                 JOIN articles a ON a.id = ae.article_id \
                 WHERE ae.entity_id = ?1 AND a.published_at >= ?2 AND a.published_at < ?3",
            )
            .bind(&[
                JsValue::from_f64(entity_id as f64),
                JsValue::from_f64(earlier_cutoff as f64),
                JsValue::from_f64(recent_cutoff as f64),
            ])?
            .first::<serde_json::Value>(None)
            .await?
            .and_then(|r| r["cnt"].as_i64())
            .unwrap_or(0);

        let trend = if earlier == 0 || recent > earlier * 12 / 10 {
            "rising"
        } else if recent < earlier * 8 / 10 {
            "declining"
        } else {
            "stable"
        };

        Ok(EntityActivitySummary {
            article_count,
            source_count,
            avg_score,
            max_score,
            first_seen_at,
            last_seen_at,
            trend: trend.into(),
        })
    }

    // ------------------------------------------------------------------
    // Artifact Registry
    // ------------------------------------------------------------------

    /// Create an artifact_registry entry for an R2-stored intelligence asset.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_artifact(&self, artifact: &NewArtifact) -> Result<i64, StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self
            .db
            .prepare(
                "INSERT INTO artifact_registry \
                 (artifact_type, entity_id, r2_key, schema_version, model, pipeline_version, metadata, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) RETURNING id",
            )
            .bind(&[
                artifact.artifact_type.as_str().into(),
                JsValue::from_f64(artifact.entity_id as f64),
                artifact.r2_key.as_str().into(),
                artifact.schema_version.as_str().into(),
                artifact.model.as_deref().map_or(JsValue::null(), |v| v.into()),
                artifact.pipeline_version.as_str().into(),
                artifact.metadata.as_deref().map_or(JsValue::null(), |v| v.into()),
                JsValue::from_f64(now as f64),
            ])?
            .first::<serde_json::Value>(None)
            .await?;

        row.and_then(|v| v["id"].as_i64())
            .ok_or_else(|| StoreError::D1("create_artifact failed: no id returned".into()))
    }

    /// List artifact_registry entries for a given entity.
    pub async fn list_artifacts_by_entity(&self, entity_id: i64, limit: u32) -> Result<Vec<ArtifactEntry>, StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT id, artifact_type, entity_id, r2_key, schema_version, model, pipeline_version, metadata, created_at \
                 FROM artifact_registry \
                 WHERE entity_id = ?1 \
                 ORDER BY created_at DESC \
                 LIMIT ?2",
            )
            .bind(&[JsValue::from_f64(entity_id as f64), JsValue::from_f64(limit as f64)])?
            .all()
            .await?
            .results()?)
    }
}

// ---- StoreBackend impl (delegates to D1Store methods) ----

#[async_trait(?Send)]
impl StoreBackend for D1Store {
    async fn get_feed(&self, id: i64) -> Result<Option<Feed>, StoreError> {
        D1Store::get_feed(self, id).await
    }

    async fn record_fetch_result(
        &self,
        feed_id: i64,
        fetched_at: i64,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<(), StoreError> {
        D1Store::record_fetch_result(self, feed_id, fetched_at, etag, last_modified).await
    }

    async fn active_rule_jsons(&self, audience_tag: &str) -> Result<Vec<String>, StoreError> {
        D1Store::active_rule_jsons(self, audience_tag).await
    }

    async fn insert_article(&self, article: &NewArticle) -> Result<Option<i64>, StoreError> {
        D1Store::insert_article(self, article).await
    }

    async fn set_ai_summary(
        &self,
        article_id: i64,
        summary: &str,
        tags_json: &str,
        vector_id: &str,
        score: f64,
    ) -> Result<(), StoreError> {
        D1Store::set_ai_summary(self, article_id, summary, tags_json, vector_id, score).await
    }

    async fn set_raw_content_r2_key(
        &self,
        article_id: i64,
        r2_key: Option<&str>,
    ) -> Result<(), StoreError> {
        D1Store::set_raw_content_r2_key(self, article_id, r2_key).await
    }

    async fn expire_old_articles(&self, now: i64, days: i64) -> Result<u64, StoreError> {
        D1Store::expire_old_articles(self, now, days).await
    }

    async fn upsert_entity(&self, name: &str, normalized: &str, entity_type: &str) -> Result<i64, StoreError> {
        D1Store::upsert_entity(self, name, normalized, entity_type).await
    }

    async fn link_article_entity(
        &self,
        article_id: i64,
        entity_id: i64,
        relevance: f64,
        context: Option<&str>,
    ) -> Result<(), StoreError> {
        D1Store::link_article_entity(self, article_id, entity_id, relevance, context).await
    }

    async fn link_entity_relation(
        &self,
        source: i64,
        target: i64,
        rtype: &str,
        confidence: f64,
    ) -> Result<(), StoreError> {
        D1Store::link_entity_relation(self, source, target, rtype, confidence).await
    }

    async fn list_entities(&self, limit: u32, offset: u32) -> Result<Vec<EntitySummary>, StoreError> {
        D1Store::list_entities(self, limit, offset).await
    }

    async fn entity_detail(&self, id: i64) -> Result<Option<EntityDetail>, StoreError> {
        D1Store::entity_detail(self, id).await
    }

    async fn entity_relations(&self, entity_id: i64, limit: u32) -> Result<Vec<RelatedEntity>, StoreError> {
        D1Store::entity_relations(self, entity_id, limit).await
    }

    async fn article_entities(&self, article_id: i64) -> Result<Vec<EntityRef>, StoreError> {
        D1Store::article_entities(self, article_id).await
    }

    async fn create_artifact(&self, artifact: &NewArtifact) -> Result<i64, StoreError> {
        D1Store::create_artifact(self, artifact).await
    }

    async fn list_artifacts_by_entity(&self, entity_id: i64, limit: u32) -> Result<Vec<ArtifactEntry>, StoreError> {
        D1Store::list_artifacts_by_entity(self, entity_id, limit).await
    }

    async fn entity_articles(
        &self,
        entity_id: i64,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<EntityArticle>, StoreError> {
        D1Store::entity_articles(self, entity_id, limit, offset).await
    }

    async fn entity_activity_summary(
        &self,
        entity_id: i64,
        now: i64,
        days: i64,
    ) -> Result<EntityActivitySummary, StoreError> {
        D1Store::entity_activity_summary(self, entity_id, now, days).await
    }

    async fn entity_signal_candidates(&self, now: i64, days: i64, limit: u32) -> Result<Vec<EntitySignalCandidate>, StoreError> {
        D1Store::entity_signal_candidates(self, now, days, limit).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn save_signal(&self, entity_id: Option<i64>, title: &str, summary: &str, confidence: f64, impact: &str, trend: &str, article_count: i64, source_count: i64, evidence_ids: &[i64], related_ids: &[i64]) -> Result<i64, StoreError> {
        D1Store::save_signal(self, entity_id, title, summary, confidence, impact, trend, article_count, source_count, evidence_ids, related_ids).await
    }

    async fn load_recent_signals(&self, limit: u32, offset: u32) -> Result<Vec<IntelligenceSignal>, StoreError> {
        D1Store::load_recent_signals(self, limit, offset).await
    }

    async fn load_signal_by_id(&self, id: i64) -> Result<Option<IntelligenceSignal>, StoreError> {
        D1Store::load_signal_by_id(self, id).await
    }

    async fn entity_signals(&self, entity_id: i64, limit: u32) -> Result<Vec<IntelligenceSignal>, StoreError> {
        D1Store::entity_signals(self, entity_id, limit).await
    }

    async fn upsert_signal_thread(&self, signal_key: &str, anchor_entity_id: Option<i64>, title: &str, status: &str) -> Result<i64, StoreError> {
        D1Store::upsert_signal_thread(self, signal_key, anchor_entity_id, title, status).await
    }

    async fn append_signal_instance(&self, thread_id: i64, confidence: f64, impact: &str, trend: &str, article_count: i64, source_count: i64) -> Result<i64, StoreError> {
        D1Store::append_signal_instance(self, thread_id, confidence, impact, trend, article_count, source_count).await
    }

    async fn update_signal_lifecycle(&self, now: i64) -> Result<(), StoreError> {
        D1Store::update_signal_lifecycle(self, now).await
    }

    async fn get_active_signal_threads(&self, limit: u32) -> Result<Vec<SignalBriefInput>, StoreError> {
        D1Store::get_active_signal_threads(self, limit).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- in_placeholders --

    #[test]
    fn in_placeholders_three() {
        assert_eq!(in_placeholders(3), "?1,?2,?3");
    }
    #[test]
    fn in_placeholders_one() {
        assert_eq!(in_placeholders(1), "?1");
    }
    #[test]
    fn in_placeholders_zero() {
        assert_eq!(in_placeholders(0), "");
    }

    // -- tag_like_pattern --

    #[test]
    fn tag_like_pattern_simple() {
        assert_eq!(tag_like_pattern("AI"), r#"%"AI"%"#);
    }
    #[test]
    fn tag_like_pattern_empty() {
        assert_eq!(tag_like_pattern(""), r#"%""%"#);
    }

    // -- is_cron_healthy --

    #[test]
    fn cron_healthy_recent() {
        assert!(is_cron_healthy(Some(1000), 3599));
    }
    #[test]
    fn cron_healthy_exact_boundary() {
        assert!(!is_cron_healthy(Some(1000), 4600));
    }
    #[test]
    fn cron_healthy_never() {
        assert!(!is_cron_healthy(None, 1000));
    }

    // -- MemoryStore integration tests --

    #[test]
    fn mem_store_loads_active_rules() {
        let store = memory::MemoryStore::new().with_rules(vec!["{\"score\":10}".into()]);
        let rules = futures::executor::block_on(store.active_rule_jsons("default")).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0], "{\"score\":10}");
    }

    #[test]
    fn mem_store_active_rules_empty_when_none() {
        let store = memory::MemoryStore::new();
        let rules = futures::executor::block_on(store.active_rule_jsons("default")).unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn mem_store_inserts_article() {
        let store = memory::MemoryStore::new();
        let article = NewArticle {
            feed_id: 1,
            guid: "guid-1".into(),
            title: "Test".into(),
            url: None,
            published_at: None,
            raw_content_r2_key: None,
        };
        let id = futures::executor::block_on(store.insert_article(&article)).unwrap();
        assert!(id.is_some());
    }

    #[test]
    fn mem_store_dedup_article() {
        let store = memory::MemoryStore::new();
        let article = NewArticle {
            feed_id: 1,
            guid: "dup-guid".into(),
            title: "Original".into(),
            url: None,
            published_at: None,
            raw_content_r2_key: None,
        };
        let id1 = futures::executor::block_on(store.insert_article(&article)).unwrap();
        assert!(id1.is_some());
        let id2 = futures::executor::block_on(store.insert_article(&article)).unwrap();
        assert!(id2.is_none(), "duplicate should return None");
    }

    #[test]
    fn mem_store_set_ai_summary() {
        let store = memory::MemoryStore::new();
        let result = futures::executor::block_on(store.set_ai_summary(42, "AI summary text", "[\"tag1\"]", "vec-42", 8.5));
        assert!(result.is_ok());
    }

    #[test]
    fn mem_store_record_fetch_result() {
        let store = memory::MemoryStore::new();
        let result = futures::executor::block_on(store.record_fetch_result(1, 1000, Some("etag-x"), Some("modified-y")));
        assert!(result.is_ok());
        assert_eq!(store.fetch_results.borrow().len(), 1);
        let (fid, _, e, lm) = store.fetch_results.borrow().first().unwrap().clone();
        assert_eq!(fid, 1);
        assert_eq!(e, Some("etag-x".into()));
        assert_eq!(lm, Some("modified-y".into()));
    }

    #[test]
    fn mem_store_returns_err_on_fail_insert() {
        let mut store = memory::MemoryStore::new();
        store.fail_insert = true;
        let article = NewArticle {
            feed_id: 1,
            guid: "err-test".into(),
            title: "Err".into(),
            url: None,
            published_at: None,
            raw_content_r2_key: None,
        };
        let result = futures::executor::block_on(store.insert_article(&article));
        assert!(result.is_err());
    }

    #[test]
    fn mem_store_returns_err_on_fail_rules() {
        let mut store = memory::MemoryStore::new();
        store.fail_rules = true;
        let result = futures::executor::block_on(store.active_rule_jsons("default"));
        assert!(result.is_err());
    }

    #[test]
    fn mem_store_returns_err_on_fail_fetch_result() {
        let mut store = memory::MemoryStore::new();
        store.fail_fetch_result = true;
        let result = futures::executor::block_on(store.record_fetch_result(1, 0, None, None));
        assert!(result.is_err());
    }

    #[test]
    fn mem_store_returns_err_on_fail_summary() {
        let mut store = memory::MemoryStore::new();
        store.fail_summary = true;
        let result = futures::executor::block_on(store.set_ai_summary(1, "summary", "[]", "vec-1", 0.0));
        assert!(result.is_err());
    }
}
