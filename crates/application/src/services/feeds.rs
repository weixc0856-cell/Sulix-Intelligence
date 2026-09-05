//! Feed application service — orchestrates the Feed CRUD use-cases (list / get
//! / create / update / soft-delete) that the API routes expose under
//! `/api/feeds`.
//!
//! Generic over the narrowest store surface — [`store::FeedQueryService`] for
//! listing, [`store::FeedRepository`] for writes and [`store::SourceRepository`]
//! for the auto-registered default source.  Zero Worker / HTTP / `js_sys` code.

use store::{Feed, NewSource, StoreError};

/// Application service for Feed CRUD use-cases.
pub struct FeedService<S> {
    store: S,
}

impl<S> FeedService<S>
where
    S: store::FeedRepository + store::FeedQueryService + store::SourceRepository,
{
    /// Wrap a store (or store-backed repository/query pair) in the service.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// List all feeds, optionally filtered by status.
    pub async fn list(&self, status_filter: Option<&str>) -> Result<Vec<Feed>, StoreError> {
        self.store.all_feeds(status_filter).await
    }

    /// Get a single feed by its primary key.
    pub async fn get(&self, id: i64) -> Result<Option<Feed>, StoreError> {
        self.store.find_feed(id).await
    }

    /// Create a feed, returning `None` when a feed with that URL already
    /// exists.  A default registry source is auto-registered for the new feed
    /// (an application invariant, mirroring the historical handler behaviour).
    pub async fn create(
        &self,
        url: &str,
        title: &str,
        category: &str,
        interval: i64,
    ) -> Result<Option<i64>, StoreError> {
        let feed_id = match self.store.insert_feed(url, title, category, interval).await? {
            Some(id) => id,
            None => return Ok(None),
        };

        // Auto-register a default source entry for governance lineage.
        let _ = self
            .store
            .save_source(&NewSource {
                source_type: "RssFeed".into(),
                feed_id: Some(feed_id),
                name: Some(title.into()),
                tier: "Tier2".into(),
                policy: "SummaryAllowed".into(),
                license: "Unknown".into(),
                license_detail: None,
                attribution: Some(title.into()),
                trust_score: None,
                retention_days: None,
                verified: false,
                notes: None,
            })
            .await;

        Ok(Some(feed_id))
    }

    /// Update a feed's editable fields; `status`, when present, is applied
    /// first (mirrors the historical handler ordering).
    pub async fn update(
        &self,
        id: i64,
        title: Option<&str>,
        category: Option<&str>,
        interval: Option<i64>,
        extraction_level: Option<&str>,
        status: Option<&str>,
    ) -> Result<(), StoreError> {
        if let Some(status) = status {
            self.store.set_feed_status(id, status).await?;
        }
        self.store.update_feed(id, title, category, interval, extraction_level).await
    }

    /// Soft-delete a feed by setting its status to `"inactive"`.
    pub async fn delete(&self, id: i64) -> Result<(), StoreError> {
        self.store.set_feed_status(id, "inactive").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use store::memory::MemoryStore;

    // MemoryStore feeds are seeded, not writable (insert/update/status return
    // "not implemented"), so the write use-cases are covered on the D1 path
    // only.  These tests pin the read contract against seeded state.

    fn feed(id: i64) -> Feed {
        Feed {
            id,
            url: format!("https://example.com/{id}/rss"),
            title: Some(format!("Example {id}")),
            category: Some("tech".into()),
            fetch_interval_sec: 3600,
            last_fetched_at: None,
            etag: None,
            last_modified: None,
            status: "active".into(),
            extraction_level: "full".into(),
        }
    }

    fn seeded() -> MemoryStore {
        MemoryStore::new().with_feed(feed(1)).with_feed(feed(2))
    }

    #[test]
    fn list_returns_seeded_feeds() {
        let svc = FeedService::new(seeded());
        let feeds = futures::executor::block_on(svc.list(None)).expect("list should succeed");
        assert_eq!(feeds.len(), 2);
    }

    #[test]
    fn get_returns_seeded_feed() {
        let svc = FeedService::new(seeded());
        let got = futures::executor::block_on(svc.get(1)).expect("get should succeed").expect("feed should exist");
        assert_eq!(got.title.as_deref(), Some("Example 1"));
    }

    #[test]
    fn get_missing_returns_none() {
        let svc = FeedService::new(seeded());
        assert!(futures::executor::block_on(svc.get(999)).expect("get should succeed").is_none());
    }
}
