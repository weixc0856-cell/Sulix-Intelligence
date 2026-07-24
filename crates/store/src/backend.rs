//! `StoreBackend` trait — abstraction over D1 so the pipeline can be
//! unit-tested with a [`MemoryStore`](crate::memory::MemoryStore).
//!
//! MVP scope: only the methods used by the feed processing pipeline.

use async_trait::async_trait;

use crate::{Feed, NewArticle, StoreError};

/// Storage backend for the feed pipeline.
///
/// Every method maps 1:1 to a D1 query.  The production implementation
/// ([`D1Store`](crate::D1Store)) wraps `worker::D1Database`; the test
/// implementation ([`MemoryStore`](crate::memory::MemoryStore)) uses
/// in-memory `HashMap`/`Vec` and supports failure injection.
#[async_trait(?Send)]
pub trait StoreBackend {
    // ---- Feeds ----

    /// Load one feed by id.
    async fn get_feed(&self, id: i64) -> Result<Option<Feed>, StoreError>;

    /// Record a fetch result (etag / last-modified) after a successful fetch.
    async fn record_fetch_result(
        &self,
        feed_id: i64,
        fetched_at: i64,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<(), StoreError>;

    // ---- Rules ----

    /// Return `rule_json` strings for every enabled rule matching `audience_tag`.
    async fn active_rule_jsons(&self, audience_tag: &str) -> Result<Vec<String>, StoreError>;

    // ---- Articles ----

    /// Insert a new article (INSERT OR IGNORE).  Returns the new row id,
    /// or `None` when the article already exists (duplicate GUID).
    async fn insert_article(&self, article: &NewArticle) -> Result<Option<i64>, StoreError>;

    /// Persist AI summarisation results.
    async fn set_ai_summary(
        &self,
        article_id: i64,
        summary: &str,
        tags_json: &str,
        vector_id: &str,
        score: f64,
    ) -> Result<(), StoreError>;

    /// Update the R2 key pointing to the article's full-text body.
    async fn set_raw_content_r2_key(
        &self,
        article_id: i64,
        r2_key: Option<&str>,
    ) -> Result<(), StoreError>;

    /// Delete articles older than `days` whose AI processing is complete.
    async fn expire_old_articles(&self, now: i64, days: i64) -> Result<u64, StoreError>;
}
