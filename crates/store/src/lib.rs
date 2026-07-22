//! D1 access layer.  Every other crate (rules, ai-pipeline, search, api)
//! talks to storage only through this crate, so backend swaps never leak
//! into business logic.
//!
//! Type definitions live in [`models`] and are re-exported from the crate
//! root so callers write `store::Feed` / `store::StoreError` etc.

mod models;
pub use models::*;

use serde::Deserialize;
use worker::wasm_bindgen::JsValue;
use worker::D1Database;

pub struct Store {
    db: D1Database,
}

impl Store {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }

    // ------------------------------------------------------------------
    // Feeds
    // ------------------------------------------------------------------

    /// Feeds due for fetch: active AND past their fetch_interval_sec.
    pub async fn feeds_due_for_fetch(&self, now: i64, category: Option<&str>) -> Result<Vec<Feed>, StoreError> {
        let (sql, _has_cat) = if category.is_some() {  // has_cat used below for bind count

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
        let (sql, has_filter) = if status_filter.is_some() {
            ("SELECT id, url, title, category, fetch_interval_sec, last_fetched_at, etag, last_modified, status, extraction_level FROM feeds WHERE status = ?1 ORDER BY last_fetched_at DESC", true)
        } else {
            ("SELECT id, url, title, category, fetch_interval_sec, last_fetched_at, etag, last_modified, status, extraction_level FROM feeds ORDER BY last_fetched_at DESC", false)
        };
        let stmt = self.db.prepare(sql);
        let stmt = if has_filter {
            stmt.bind(&[status_filter.unwrap().into()])?
        } else {
            stmt
        };
        Ok(stmt.all().await?.results()?)
    }

    pub async fn get_feed(&self, id: i64) -> Result<Option<Feed>, StoreError> {
        let stmt = self.db.prepare(
            "SELECT id, url, title, category, fetch_interval_sec, last_fetched_at, etag, last_modified, status, extraction_level FROM feeds WHERE id = ?1",
        ).bind(&[JsValue::from_f64(id as f64)])?;
        Ok(stmt.first::<Feed>(None).await?)
    }

    pub async fn insert_feed(&self, url: &str, title: &str, category: &str, interval: i64) -> Result<Option<i64>, StoreError> {
        let stmt = self.db.prepare(
            "INSERT OR IGNORE INTO feeds (url, title, category, fetch_interval_sec) VALUES (?1, ?2, ?3, ?4)",
        ).bind(&[url.into(), title.into(), category.into(), JsValue::from_f64(interval as f64)])?;
        stmt.run().await?;
        let q = self.db.prepare("SELECT id FROM feeds WHERE url = ?1").bind(&[url.into()])?;
        let row = q.first::<serde_json::Value>(None).await?;
        Ok(row.and_then(|v| v.get("id").and_then(|id| id.as_i64())))
    }

    /// Dynamic update: only non-None fields are applied.
    pub async fn update_feed(&self, id: i64, title: Option<&str>, category: Option<&str>, interval: Option<i64>, extraction_level: Option<&str>) -> Result<(), StoreError> {
        let mut parts: Vec<String> = Vec::new();
        let mut vals: Vec<JsValue> = Vec::new();
        if let Some(v) = title          { parts.push("title = ?".into()); vals.push(v.into()); }
        if let Some(v) = category       { parts.push("category = ?".into()); vals.push(v.into()); }
        if let Some(v) = interval       { parts.push("fetch_interval_sec = ?".into()); vals.push(JsValue::from_f64(v as f64)); }
        if let Some(v) = extraction_level { parts.push("extraction_level = ?".into()); vals.push(v.into()); }
        if parts.is_empty() { return Ok(()); }
        vals.push(JsValue::from_f64(id as f64));
        self.db.prepare(format!("UPDATE feeds SET {} WHERE id = ?", parts.join(", "))).bind(&vals)?.run().await?;
        Ok(())
    }

    pub async fn set_feed_status(&self, id: i64, status: &str) -> Result<(), StoreError> {
        self.db.prepare("UPDATE feeds SET status = ?1 WHERE id = ?2")
            .bind(&[status.into(), JsValue::from_f64(id as f64)])?.run().await?;
        Ok(())
    }

    pub async fn record_fetch_result(&self, feed_id: i64, fetched_at: i64, etag: Option<&str>, last_modified: Option<&str>) -> Result<(), StoreError> {
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
        self.db.prepare(
            "INSERT OR IGNORE INTO articles (feed_id, guid, title, url, published_at, raw_content_r2_key) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        ).bind(&[
            JsValue::from_f64(article.feed_id as f64),
            article.guid.clone().into(),
            article.title.clone().into(),
            article.url.clone().map_or(JsValue::null(), |v| v.into()),
            article.published_at.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
            article.raw_content_r2_key.clone().map_or(JsValue::null(), |v| v.into()),
        ])?.run().await?;
        let q = self.db.prepare("SELECT id FROM articles WHERE feed_id = ?1 AND guid = ?2")
            .bind(&[JsValue::from_f64(article.feed_id as f64), article.guid.clone().into()])?;
        let row = q.first::<serde_json::Value>(None).await?;
        Ok(row.and_then(|v| v.get("id").and_then(|id| id.as_i64())))
    }

    pub async fn set_ai_summary(&self, article_id: i64, summary: &str, tags_json: &str, vector_id: &str, score: f64) -> Result<(), StoreError> {
        self.db.prepare(
            "UPDATE articles SET ai_summary = ?1, ai_tags = ?2, vector_id = ?3, score = ?4 WHERE id = ?5",
        ).bind(&[summary.into(), tags_json.into(), vector_id.into(), JsValue::from_f64(score), JsValue::from_f64(article_id as f64)])?.run().await?;
        Ok(())
    }

    pub async fn set_raw_content_r2_key(&self, article_id: i64, r2_key: Option<&str>) -> Result<(), StoreError> {
        self.db.prepare("UPDATE articles SET raw_content_r2_key = ?1 WHERE id = ?2")
            .bind(&[r2_key.into(), JsValue::from_f64(article_id as f64)])?.run().await?;
        Ok(())
    }

    pub async fn latest_articles(&self, limit: u32, offset: u32) -> Result<Vec<PendingArticle>, StoreError> {
        Ok(self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles ORDER BY published_at DESC LIMIT ?1 OFFSET ?2",
        ).bind(&[JsValue::from_f64(limit as f64), JsValue::from_f64(offset as f64)])?.all().await?.results()?)
    }

    pub async fn article_count(&self) -> Result<i64, StoreError> {
        let row = self.db.prepare(
            "SELECT COUNT(*) AS cnt FROM articles",
        ).first::<serde_json::Value>(None).await?;
        Ok(row.and_then(|v| v["cnt"].as_i64()).unwrap_or(0))
    }

    pub async fn trending_articles(&self, limit: u32, offset: u32) -> Result<Vec<PendingArticle>, StoreError> {
        Ok(self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles WHERE score != 0 ORDER BY score DESC, published_at DESC LIMIT ?1 OFFSET ?2",
        ).bind(&[JsValue::from_f64(limit as f64), JsValue::from_f64(offset as f64)])?.all().await?.results()?)
    }

    pub async fn trending_count(&self) -> Result<i64, StoreError> {
        let row = self.db.prepare(
            "SELECT COUNT(*) AS cnt FROM articles WHERE score != 0",
        ).first::<serde_json::Value>(None).await?;
        Ok(row.and_then(|v| v["cnt"].as_i64()).unwrap_or(0))
    }

    pub async fn article_by_id(&self, id: i64) -> Result<Option<Article>, StoreError> {
        Ok(self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles WHERE id = ?1",
        ).bind(&[JsValue::from_f64(id as f64)])?.first::<Article>(None).await?)
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

    pub async fn articles_by_tag(&self, tag: &str, limit: u32) -> Result<Vec<PendingArticle>, StoreError> {
        let pattern = format!("%\"{}\"%", tag);
        Ok(self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles WHERE ai_tags LIKE ?1 ORDER BY published_at DESC LIMIT ?2",
        ).bind(&[pattern.into(), JsValue::from_f64(limit as f64)])?.all().await?.results()?)
    }

    pub async fn articles_by_category(&self, category: &str, limit: u32) -> Result<Vec<PendingArticle>, StoreError> {
        Ok(self.db.prepare(
            "SELECT a.id, a.feed_id, a.guid, a.title, a.url, a.published_at, a.ai_summary, a.ai_tags, a.score FROM articles a JOIN feeds f ON f.id = a.feed_id WHERE f.category = ?1 ORDER BY a.published_at DESC LIMIT ?2",
        ).bind(&[category.into(), JsValue::from_f64(limit as f64)])?.all().await?.results()?)
    }

    pub async fn categories_summary(&self) -> Result<Vec<(String, i64)>, StoreError> {
        #[derive(Deserialize)]
        struct Row { category: String, article_count: i64 }
        let rows: Vec<Row> = self.db.prepare(
            "SELECT f.category, COUNT(a.id) AS article_count FROM feeds f LEFT JOIN articles a ON a.feed_id = f.id WHERE f.category IS NOT NULL AND f.category != '' GROUP BY f.category ORDER BY article_count DESC",
        ).all().await?.results()?;
        Ok(rows.into_iter().map(|r| (r.category, r.article_count)).collect())
    }

    /// Find articles sharing tags with a given article, ordered by match
    /// count desc then recency.  Returns empty when source has no tags.
    pub async fn related_articles(&self, article_id: i64, limit: u32) -> Result<Vec<PendingArticle>, StoreError> {
        let src = self.db.prepare("SELECT ai_tags FROM articles WHERE id = ?1")
            .bind(&[JsValue::from_f64(article_id as f64)])?;
        let tags_json = match src.first::<String>(None).await? {
            Some(t) => t, None => return Ok(Vec::new()),
        };
        let tags: Vec<String> = match serde_json::from_str(&tags_json) {
            Ok(t) => t, Err(_) => return Ok(Vec::new()),
        };
        if tags.is_empty() { return Ok(Vec::new()); }
        let conds: Vec<String> = tags.iter().map(|t| format!("ai_tags LIKE '%\"{}%'", t.replace('\'', "''"))).collect();
        let sql = format!(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles WHERE id != ?1 AND ({}) ORDER BY ({} DESC), published_at DESC LIMIT ?2",
            conds.join(" OR "),
            conds.iter().map(|c| format!("CASE WHEN {} THEN 1 ELSE 0 END", c)).collect::<Vec<_>>().join(" + "),
        );
        Ok(self.db.prepare(&sql).bind(&[JsValue::from_f64(article_id as f64), JsValue::from_f64(limit as f64)])?.all().await?.results()?)
    }

    // ------------------------------------------------------------------
    // Aggregations
    // ------------------------------------------------------------------

    pub async fn tags_summary(&self) -> Result<Vec<(String, i64)>, StoreError> {
        #[derive(Deserialize)]
        struct Row { ai_tags: String }
        let rows: Vec<Row> = self.db.prepare(
            "SELECT ai_tags FROM articles WHERE ai_tags IS NOT NULL AND ai_tags != '[]'",
        ).all().await?.results()?;
        let mut map: std::collections::BTreeMap<String, i64> = std::collections::BTreeMap::new();
        for row in &rows {
            if let Ok(tags) = serde_json::from_str::<Vec<String>>(&row.ai_tags) {
                for tag in tags { *map.entry(tag).or_default() += 1; }
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

    pub async fn article_trend(&self, days: i64) -> Result<Vec<DayCount>, StoreError> {
        Ok(self.db.prepare(
            "SELECT DATE(published_at, 'unixepoch') AS day, COUNT(*) AS cnt FROM articles WHERE published_at IS NOT NULL GROUP BY day ORDER BY day DESC LIMIT ?1",
        ).bind(&[JsValue::from_f64(days as f64)])?.all().await?.results()?)
    }

    /// Get articles that still need AI summarization, oldest first.
    /// Batch size limits per call to stay within Workers CPU time budget.
    pub async fn pending_ai_articles(&self, batch_size: u32) -> Result<Vec<PendingArticle>, StoreError> {
        Ok(self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score, raw_content_r2_key
             FROM articles WHERE (ai_summary IS NULL OR ai_summary = '')
             ORDER BY published_at ASC LIMIT ?1",
        ).bind(&[JsValue::from_f64(batch_size as f64)])?.all().await?.results()?)
    }

    /// Mark an article as having been processed by AI (set summary + tags + score).
    pub async fn mark_ai_processed(&self, id: i64, summary: &str, tags_json: &str, vector_id: &str, score: f64) -> Result<(), StoreError> {
        self.db.prepare(
            "UPDATE articles SET ai_summary = ?1, ai_tags = ?2, vector_id = ?3, score = ?4 WHERE id = ?5",
        ).bind(&[summary.into(), tags_json.into(), vector_id.into(), JsValue::from_f64(score), JsValue::from_f64(id as f64)])?.run().await?;
        Ok(())
    }

    /// Delete articles older than `days` whose AI processing is complete.
    /// `now` should be the current unix timestamp (seconds), typically
    /// passed from the caller's js_sys::Date::now().
    /// Protects D1 from unbounded growth as feed volume increases.
    pub async fn expire_old_articles(&self, now: i64, days: i64) -> Result<u64, StoreError> {
        let cutoff = now - days * 86400;
        let stmt = self.db.prepare(
            "DELETE FROM articles WHERE published_at < ?1 AND ai_summary != '' AND ai_summary IS NOT NULL",
        ).bind(&[JsValue::from_f64(cutoff as f64)])?;
        stmt.run().await?;
        Ok(0)
    }

    // ------------------------------------------------------------------
    // Rules
    // ------------------------------------------------------------------

    pub async fn active_rule_jsons(&self, audience_tag: &str) -> Result<Vec<String>, StoreError> {
        #[derive(Deserialize)]
        struct Row { rule_json: String }
        let rows: Vec<Row> = self.db.prepare(
            "SELECT rule_json FROM filter_rules WHERE audience_tag = ?1 AND enabled = 1",
        ).bind(&[audience_tag.into()])?.all().await?.results()?;
        Ok(rows.into_iter().map(|r| r.rule_json).collect())
    }
}
