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

pub struct Store {
    db: D1Database,
}

impl Store {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }

    pub async fn active_feeds(&self) -> Result<Vec<Feed>, StoreError> {
        let stmt = self
            .db
            .prepare("SELECT id, url, title, category, fetch_interval_sec, last_fetched_at, etag, last_modified, status FROM feeds WHERE status = 'active'");
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
}
