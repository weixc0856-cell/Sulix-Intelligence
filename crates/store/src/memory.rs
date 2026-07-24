//! In-memory [`StoreBackend`](crate::StoreBackend) implementation for tests.
//!
//! Uses `HashMap` / `Vec` instead of D1.  Supports failure injection so
//! pipeline error-handling paths can be exercised without a database.

use async_trait::async_trait;
use std::cell::RefCell;
use std::collections::HashMap;

use crate::{backend::StoreBackend, Feed, NewArticle, StoreError};

/// In-memory store with failure-injection flags.
///
/// Uses `RefCell` for interior mutability — safe because the trait is
/// `#[async_trait(?Send)]` and tests run on a single thread.
pub struct MemoryStore {
    pub feeds: HashMap<i64, Feed>,
    pub rules: Vec<String>,

    // RefCell for interior mutability (trait takes &self)
    articles: RefCell<Vec<NewArticle>>,
    next_article_id: RefCell<i64>,
    summaries: RefCell<HashMap<i64, String>>,
    r2_keys: RefCell<HashMap<i64, Option<String>>>,
    pub fetch_results: RefCell<Vec<(i64, i64, Option<String>, Option<String>)>>,

    /// When `true`, `insert_article` returns `Err`.
    pub fail_insert: bool,
    /// When `true`, `active_rule_jsons` returns `Err`.
    pub fail_rules: bool,
    /// When `true`, `set_ai_summary` returns `Err`.
    pub fail_summary: bool,
    /// When `true`, `record_fetch_result` returns `Err`.
    pub fail_fetch_result: bool,
    /// When `true`, `set_raw_content_r2_key` returns `Err`.
    pub fail_r2_key: bool,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            feeds: HashMap::new(),
            articles: RefCell::new(Vec::new()),
            rules: Vec::new(),
            summaries: RefCell::new(HashMap::new()),
            r2_keys: RefCell::new(HashMap::new()),
            fetch_results: RefCell::new(Vec::new()),
            next_article_id: RefCell::new(1),
            fail_insert: false,
            fail_rules: false,
            fail_summary: false,
            fail_fetch_result: false,
            fail_r2_key: false,
        }
    }

    /// Builder-style: set the rules that `active_rule_jsons` returns.
    pub fn with_rules(mut self, rules: Vec<String>) -> Self {
        self.rules = rules;
        self
    }

    /// Builder-style: insert a feed into the store.
    pub fn with_feed(mut self, feed: Feed) -> Self {
        self.feeds.insert(feed.id, feed);
        self
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait(?Send)]
impl StoreBackend for MemoryStore {
    async fn get_feed(&self, id: i64) -> Result<Option<Feed>, StoreError> {
        Ok(self.feeds.get(&id).cloned())
    }

    async fn record_fetch_result(
        &self,
        feed_id: i64,
        fetched_at: i64,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<(), StoreError> {
        if self.fail_fetch_result {
            return Err(StoreError::D1("injected fetch result failure".into()));
        }
        self.fetch_results.borrow_mut().push((
            feed_id,
            fetched_at,
            etag.map(|s| s.to_string()),
            last_modified.map(|s| s.to_string()),
        ));
        Ok(())
    }

    async fn active_rule_jsons(&self, _audience_tag: &str) -> Result<Vec<String>, StoreError> {
        if self.fail_rules {
            return Err(StoreError::D1("injected rules failure".into()));
        }
        Ok(self.rules.clone())
    }

    async fn insert_article(&self, article: &NewArticle) -> Result<Option<i64>, StoreError> {
        if self.fail_insert {
            return Err(StoreError::D1("injected insert failure".into()));
        }
        // Dedup by feed_id + guid (same as D1's INSERT OR IGNORE)
        let dup = self.articles.borrow().iter().any(|a| a.feed_id == article.feed_id && a.guid == article.guid);
        if dup {
            return Ok(None);
        }
        let id = *self.next_article_id.borrow();
        *self.next_article_id.borrow_mut() = id + 1;
        self.articles.borrow_mut().push(NewArticle {
            feed_id: article.feed_id,
            guid: article.guid.clone(),
            title: article.title.clone(),
            url: article.url.clone(),
            published_at: article.published_at,
            raw_content_r2_key: article.raw_content_r2_key.clone(),
        });
        Ok(Some(id))
    }

    async fn set_ai_summary(
        &self,
        article_id: i64,
        summary: &str,
        _tags_json: &str,
        _vector_id: &str,
        _score: f64,
    ) -> Result<(), StoreError> {
        if self.fail_summary {
            return Err(StoreError::D1("injected summary failure".into()));
        }
        self.summaries.borrow_mut().insert(article_id, summary.to_string());
        Ok(())
    }

    async fn set_raw_content_r2_key(
        &self,
        article_id: i64,
        r2_key: Option<&str>,
    ) -> Result<(), StoreError> {
        if self.fail_r2_key {
            return Err(StoreError::D1("injected r2 key failure".into()));
        }
        self.r2_keys.borrow_mut().insert(article_id, r2_key.map(|s| s.to_string()));
        Ok(())
    }

    async fn expire_old_articles(&self, _now: i64, _days: i64) -> Result<u64, StoreError> {
        Ok(0)
    }
}
