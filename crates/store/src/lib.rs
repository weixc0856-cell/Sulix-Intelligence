//! D1 access layer. Every other crate (rules, ai-pipeline, search, api)
//! talks to storage only through this crate, so backend swaps (e.g. an
//! external search service later) never leak into business logic.
//!
//! NOTE: D1 bind params must be f64-compatible values because wasm-bindgen
//! converts i64 → BigInt, which D1's JS API does not accept. All numeric
//! D1 parameters are cast to f64 before binding.

use serde::Deserialize;
use serde::Serialize;
use worker::D1Database;
use worker::wasm_bindgen::JsValue;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("d1 error: {0}")]
    D1(String),
}

impl From<worker::Error> for StoreError {
    fn from(e: worker::Error) -> Self {
        StoreError::D1(e.to_string())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Feed {
    pub id: i64,
    pub url: String,
    pub title: Option<String>,
    pub category: Option<String>,
    pub fetch_interval_sec: i64,
    pub last_fetched_at: Option<i64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub status: String,
    /// 'summary_only' (default) — only use RSS entry summary/content.
    /// 'full_text' — opt-in, fetches the article URL for full-text extraction.
    pub extraction_level: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewArticle {
    pub feed_id: i64,
    pub guid: String,
    pub title: String,
    pub url: Option<String>,
    pub published_at: Option<i64>,
    pub raw_content_r2_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Article {
    pub id: i64,
    pub feed_id: i64,
    pub guid: String,
    pub title: String,
    pub url: Option<String>,
    pub published_at: Option<i64>,
    pub ai_summary: String,
    pub ai_tags: Option<String>,
    pub score: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeedStats {
    pub id: i64,
    pub title: Option<String>,
    pub url: String,
    pub category: Option<String>,
    pub status: String,
    pub last_fetched_at: Option<i64>,
    pub article_count: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HealthStats {
    pub feed_count: i64,
    pub active_feed_count: i64,
    pub article_count: i64,
    /// Max last_fetched_at across all feeds -- a proxy for "last cron run",
    /// since every scheduled run touches this via record_fetch_result.
    pub last_cron_run_at: Option<i64>,
}

pub struct Store {
    db: D1Database,
}

impl Store {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }

    /// Feeds whose status is 'active' AND whose last_fetched_at is either
    /// NULL (never fetched) or more than fetch_interval_sec seconds ago.
    /// Optional category filter: pass Some("tech") to only return feeds in
    /// that category; pass None to skip filtering.
    pub async fn feeds_due_for_fetch(&self, now: i64, category: Option<&str>) -> Result<Vec<Feed>, StoreError> {
        let sql = match category {
            Some(_) => "SELECT id, url, title, category, fetch_interval_sec, last_fetched_at, etag, last_modified, status, extraction_level
                        FROM feeds WHERE status = 'active' AND category = ?1
                        AND (last_fetched_at IS NULL OR ?2 - last_fetched_at >= fetch_interval_sec)",
            None => "SELECT id, url, title, category, fetch_interval_sec, last_fetched_at, etag, last_modified, status, extraction_level
                     FROM feeds WHERE status = 'active'
                     AND (last_fetched_at IS NULL OR ?1 - last_fetched_at >= fetch_interval_sec)",
        };
        let stmt = self.db.prepare(sql);
        let stmt = if let Some(cat) = category {
            stmt.bind(&[cat.into(), JsValue::from_f64(now as f64)])?
        } else {
            stmt.bind(&[JsValue::from_f64(now as f64)])?
        };
        let result = stmt.all().await?;
        Ok(result.results::<Feed>()?)
    }

    /// Called after every fetch attempt (whether it returned new content or
    /// a 304): updates last_fetched_at always, and etag/last_modified only
    /// when the fetch actually returned new values to remember for next
    /// time's conditional request.
    pub async fn record_fetch_result(
        &self,
        feed_id: i64,
        fetched_at: i64,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<(), StoreError> {
        let stmt = self.db.prepare(
            "UPDATE feeds SET last_fetched_at = ?1, etag = COALESCE(?2, etag), last_modified = COALESCE(?3, last_modified) WHERE id = ?4",
        );
        let stmt = stmt.bind(&[
            JsValue::from_f64(fetched_at as f64),
            etag.into(),
            last_modified.into(),
            JsValue::from_f64(feed_id as f64),
        ])?;
        stmt.run().await?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Feed CRUD (management API)
    // ------------------------------------------------------------------

    /// List all feeds regardless of status, newest first.  Supports an
    /// optional status filter ("active", "inactive", etc.).
    pub async fn all_feeds(&self, status_filter: Option<&str>) -> Result<Vec<Feed>, StoreError> {
        let (sql, bind) = if status_filter.is_some() {
            ("SELECT id, url, title, category, fetch_interval_sec, last_fetched_at, etag, last_modified, status, extraction_level FROM feeds WHERE status = ?1 ORDER BY last_fetched_at DESC", true)
        } else {
            ("SELECT id, url, title, category, fetch_interval_sec, last_fetched_at, etag, last_modified, status, extraction_level FROM feeds ORDER BY last_fetched_at DESC", false)
        };
        let stmt = self.db.prepare(sql);
        let stmt = if bind {
            stmt.bind(&[status_filter.unwrap().into()])?
        } else {
            stmt
        };
        let result = stmt.all().await?;
        Ok(result.results::<Feed>()?)
    }

    /// Get a single feed by id.  Returns None when not found.
    pub async fn get_feed(&self, id: i64) -> Result<Option<Feed>, StoreError> {
        let stmt = self.db.prepare(
            "SELECT id, url, title, category, fetch_interval_sec, last_fetched_at, etag, last_modified, status, extraction_level FROM feeds WHERE id = ?1",
        );
        let stmt = stmt.bind(&[JsValue::from_f64(id as f64)])?;
        Ok(stmt.first::<Feed>(None).await?)
    }

    /// Insert a new feed.  Returns the new row id on success, or None
    /// if a feed with this url already exists (due to UNIQUE(url)).
    pub async fn insert_feed(
        &self,
        url: &str,
        title: &str,
        category: &str,
        fetch_interval_sec: i64,
    ) -> Result<Option<i64>, StoreError> {
        let stmt = self.db.prepare(
            "INSERT OR IGNORE INTO feeds (url, title, category, fetch_interval_sec) VALUES (?1, ?2, ?3, ?4)",
        );
        let stmt = stmt.bind(&[
            url.into(),
            title.into(),
            category.into(),
            JsValue::from_f64(fetch_interval_sec as f64),
        ])?;
        stmt.run().await?;

        // Query back the id (new or existing).
        let q = self.db.prepare("SELECT id FROM feeds WHERE url = ?1");
        let q = q.bind(&[url.into()])?;
        Ok(q.first::<i64>(None).await?)
    }

    /// Update feed fields.  Only non-None fields are applied.
    pub async fn update_feed(
        &self,
        id: i64,
        title: Option<&str>,
        category: Option<&str>,
        fetch_interval_sec: Option<i64>,
        extraction_level: Option<&str>,
    ) -> Result<(), StoreError> {
        let mut parts: Vec<String> = Vec::new();
        let mut bind_vals: Vec<JsValue> = Vec::new();

        if let Some(v) = title {
            parts.push("title = ?".to_string());
            bind_vals.push(v.into());
        }
        if let Some(v) = category {
            parts.push("category = ?".to_string());
            bind_vals.push(v.into());
        }
        if let Some(v) = fetch_interval_sec {
            parts.push("fetch_interval_sec = ?".to_string());
            bind_vals.push(JsValue::from_f64(v as f64));
        }
        if let Some(v) = extraction_level {
            parts.push("extraction_level = ?".to_string());
            bind_vals.push(v.into());
        }

        if parts.is_empty() {
            return Ok(()); // nothing to update
        }

        let sql = format!(
            "UPDATE feeds SET {} WHERE id = ?",
            parts.join(", ")
        );
        bind_vals.push(JsValue::from_f64(id as f64));

        let stmt = self.db.prepare(&sql);
        let stmt = stmt.bind(&bind_vals)?;
        stmt.run().await?;
        Ok(())
    }

    /// Soft-delete: set status to 'inactive' rather than removing the row.
    /// Re-enable by calling set_feed_status with 'active'.
    pub async fn set_feed_status(&self, id: i64, status: &str) -> Result<(), StoreError> {
        let stmt = self.db.prepare("UPDATE feeds SET status = ?1 WHERE id = ?2");
        let stmt = stmt.bind(&[status.into(), JsValue::from_f64(id as f64)])?;
        stmt.run().await?;
        Ok(())
    }

    /// Insert an article; relies on the UNIQUE(feed_id, guid) constraint to
    /// silently no-op on duplicates (INSERT OR IGNORE), which is what makes
    /// re-fetching the same feed idempotent.
    ///
    /// Returns `Some(article_id)` if the article exists in the database
    /// after the call (whether newly inserted or a pre-existing duplicate).
    /// Returns `None` only on database errors.
    pub async fn insert_article(&self, article: &NewArticle) -> Result<Option<i64>, StoreError> {
        let stmt = self.db.prepare(
            "INSERT OR IGNORE INTO articles (feed_id, guid, title, url, published_at, raw_content_r2_key)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        );
        let stmt = stmt.bind(&[
            JsValue::from_f64(article.feed_id as f64),
            article.guid.clone().into(),
            article.title.clone().into(),
            article.url.clone().into(),
            article.published_at.map(|v| JsValue::from_f64(v as f64)).unwrap_or(JsValue::null()),
            article.raw_content_r2_key.clone().into(),
        ])?;
        stmt.run().await?;

        // Query back the article id regardless of insert vs ignore
        let q = self.db.prepare("SELECT id FROM articles WHERE feed_id = ?1 AND guid = ?2");
        let q = q.bind(&[
            JsValue::from_f64(article.feed_id as f64),
            article.guid.clone().into(),
        ])?;
        let row = q.first::<i64>(None).await?;
        Ok(row)
    }

    /// Load active filter rules as raw JSON strings for a given audience.
    /// Callers parse into `rules::Rule` via serde_json. Returns empty vec
    /// when no rules are configured (the pipeline still works, just with
    /// a default score of 0).
    pub async fn active_rule_jsons(&self, audience_tag: &str) -> Result<Vec<String>, StoreError> {
        let stmt = self.db.prepare(
            "SELECT rule_json FROM filter_rules WHERE audience_tag = ?1 AND enabled = 1",
        );
        let stmt = stmt.bind(&[audience_tag.into()])?;
        let result = stmt.all().await?;
        #[derive(Deserialize)]
        struct Row { rule_json: String }
        let rows: Vec<Row> = result.results()?;
        Ok(rows.into_iter().map(|r| r.rule_json).collect())
    }

    pub async fn set_ai_summary(
        &self,
        article_id: i64,
        summary: &str,
        tags_json: &str,
        vector_id: &str,
        score: f64,
    ) -> Result<(), StoreError> {
        let stmt = self.db.prepare(
            "UPDATE articles SET ai_summary = ?1, ai_tags = ?2, vector_id = ?3, score = ?4 WHERE id = ?5",
        );
        let stmt = stmt.bind(&[
            summary.into(),
            tags_json.into(),
            vector_id.into(),
            JsValue::from_f64(score),
            JsValue::from_f64(article_id as f64),
        ])?;
        stmt.run().await?;
        Ok(())
    }

    pub async fn latest_articles(&self, limit: u32) -> Result<Vec<Article>, StoreError> {
        let stmt = self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score
             FROM articles ORDER BY published_at DESC LIMIT ?1",
        );
        let stmt = stmt.bind(&[JsValue::from_f64(limit as f64)])?;
        let result = stmt.all().await?;
        Ok(result.results::<Article>()?)
    }

    /// Top-scored articles for the Trending page.  Filters to articles with
    /// non-zero score so random noise (score=0) doesn't clutter the list.
    pub async fn trending_articles(&self, limit: u32) -> Result<Vec<Article>, StoreError> {
        let stmt = self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score
             FROM articles WHERE score != 0
             ORDER BY score DESC, published_at DESC LIMIT ?1",
        );
        let stmt = stmt.bind(&[JsValue::from_f64(limit as f64)])?;
        let result = stmt.all().await?;
        Ok(result.results::<Article>()?)
    }

    /// Aggregate all unique AI tags across articles with their counts.
    /// Tags are stored as JSON arrays in ai_tags — this pulls them all,
    /// parses server-side, and aggregates.  Returns empty vec when no
    /// articles have been processed yet.
    pub async fn tags_summary(&self) -> Result<Vec<(String, i64)>, StoreError> {
        let stmt = self.db.prepare(
            "SELECT ai_tags FROM articles WHERE ai_tags IS NOT NULL AND ai_tags != '[]'",
        );
        let result = stmt.all().await?;
        #[derive(Deserialize)]
        struct Row { ai_tags: String }
        let rows: Vec<Row> = result.results()?;

        // Aggregate in a BTreeMap for deterministic order
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

    /// Fetch articles that contain a specific tag in their ai_tags JSON.
    /// Uses simple LIKE '%"tag"%' which is correct for JSON arrays because
    /// each tag value appears as "tag" (with surrounding quotes) in the
    /// serialized JSON string.
    pub async fn articles_by_tag(&self, tag: &str, limit: u32) -> Result<Vec<Article>, StoreError> {
        let pattern = format!("%\"{}\"%", tag);
        let stmt = self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score
             FROM articles
             WHERE ai_tags LIKE ?1
             ORDER BY published_at DESC
             LIMIT ?2",
        );
        let stmt = stmt.bind(&[pattern.into(), JsValue::from_f64(limit as f64)])?;
        let result = stmt.all().await?;
        Ok(result.results::<Article>()?)
    }

    /// After full-text extraction, persist the R2 object key back to the
    /// article row so it's referenceable without re-fetching.  No-op when
    /// r2_key is None (cleanup / extraction_level not set to full_text).
    pub async fn set_raw_content_r2_key(&self, article_id: i64, r2_key: Option<&str>) -> Result<(), StoreError> {
        let stmt = self.db.prepare(
            "UPDATE articles SET raw_content_r2_key = ?1 WHERE id = ?2",
        );
        let stmt = stmt.bind(&[
            r2_key.into(),
            JsValue::from_f64(article_id as f64),
        ])?;
        stmt.run().await?;
        Ok(())
    }

    /// Find articles related to a given article by shared tags.  Parses
    /// the source article's ai_tags JSON, then searches for other articles
    /// that contain any of those tags via LIKE.  Results ordered by how
    /// many tags match (desc), then recency.  Returns empty vec when the
    /// source article has no tags or no matches.
    pub async fn related_articles(&self, article_id: i64, limit: u32) -> Result<Vec<Article>, StoreError> {
        // First get the source article's tags
        let src = self.db.prepare("SELECT ai_tags FROM articles WHERE id = ?1");
        let src = src.bind(&[JsValue::from_f64(article_id as f64)])?;
        let row = src.first::<String>(None).await?;
        let tags_json = match row {
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

        // Build LIKE conditions for each tag
        let conditions: Vec<String> = tags
            .iter()
            .map(|t| format!("ai_tags LIKE '%\"{}%'", t.replace('\'', "''")))
            .collect();
        let sql = format!(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score
             FROM articles
             WHERE id != ?1 AND ({})
             ORDER BY ({} DESC), published_at DESC
             LIMIT ?2",
            conditions.join(" OR "),
            conditions
                .iter()
                .map(|c| format!("CASE WHEN {} THEN 1 ELSE 0 END", c))
                .collect::<Vec<_>>()
                .join(" + "),
        );

        // Bind: ?1 = article_id, ?2 = limit
        let stmt = self.db.prepare(&sql);
        let stmt = stmt.bind(&[
            JsValue::from_f64(article_id as f64),
            JsValue::from_f64(limit as f64),
        ])?;
        let result = stmt.all().await?;
        Ok(result.results::<Article>()?)
    }

    /// Aggregate unique categories from feeds with their article counts.
    pub async fn categories_summary(&self) -> Result<Vec<(String, i64)>, StoreError> {
        let stmt = self.db.prepare(
            "SELECT f.category, COUNT(a.id) AS article_count
             FROM feeds f
             LEFT JOIN articles a ON a.feed_id = f.id
             WHERE f.category IS NOT NULL AND f.category != ''
             GROUP BY f.category
             ORDER BY article_count DESC",
        );
        let result = stmt.all().await?;
        #[derive(Deserialize)]
        struct Row { category: String, article_count: i64 }
        let rows: Vec<Row> = result.results()?;
        Ok(rows.into_iter().map(|r| (r.category, r.article_count)).collect())
    }

    /// Fetch articles belonging to a specific feed category (e.g. 'ai', 'security').
    pub async fn articles_by_category(&self, category: &str, limit: u32) -> Result<Vec<Article>, StoreError> {
        let stmt = self.db.prepare(
            "SELECT a.id, a.feed_id, a.guid, a.title, a.url, a.published_at, a.ai_summary, a.ai_tags, a.score
             FROM articles a
             JOIN feeds f ON f.id = a.feed_id
             WHERE f.category = ?1
             ORDER BY a.published_at DESC
             LIMIT ?2",
        );
        let stmt = stmt.bind(&[category.into(), JsValue::from_f64(limit as f64)])?;
        let result = stmt.all().await?;
        Ok(result.results::<Article>()?)
    }

    pub async fn article_by_id(&self, id: i64) -> Result<Option<Article>, StoreError> {
        let stmt = self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score
             FROM articles WHERE id = ?1",
        );
        let stmt = stmt.bind(&[JsValue::from_f64(id as f64)])?;
        let result = stmt.first::<Article>(None).await?;
        Ok(result)
    }

    /// Feed-level stats for the dashboard: article count and last fetch time
    /// per feed, joined from feeds + articles so inactive feeds show 0.
    pub async fn feed_stats(&self) -> Result<Vec<FeedStats>, StoreError> {
        let stmt = self.db.prepare(
            "SELECT f.id, f.title, f.url, f.category, f.status, f.last_fetched_at,
                    COUNT(a.id) AS article_count
             FROM feeds f
             LEFT JOIN articles a ON a.feed_id = f.id
             GROUP BY f.id
             ORDER BY f.last_fetched_at DESC",
        );
        let result = stmt.all().await?;
        Ok(result.results::<FeedStats>()?)
    }

    /// Backs the /api/health endpoint.  Uses max(last_fetched_at) as a
    /// proxy for "last cron run" -- every scheduled cycle calls
    /// record_fetch_result, so a recent last_fetched_at means the cron
    /// pipeline is alive.
    pub async fn health_stats(&self) -> Result<HealthStats, StoreError> {
        let stmt = self.db.prepare(
            "SELECT
               (SELECT COUNT(*) FROM feeds) AS feed_count,
               (SELECT COUNT(*) FROM feeds WHERE status = 'active') AS active_feed_count,
               (SELECT COUNT(*) FROM articles) AS article_count,
               (SELECT MAX(last_fetched_at) FROM feeds) AS last_cron_run_at",
        );
        let result = stmt.first::<HealthStats>(None).await?;
        result.ok_or_else(|| StoreError::D1("health_stats query returned no row".into()))
    }
}
