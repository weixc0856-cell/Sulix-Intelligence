use async_trait::async_trait;

use crate::{Feed, StoreError};

/// Feed aggregate persistence.
///
/// `save_feed` covers insert/update of a whole aggregate; `find_feed` loads by
/// primary key; `record_fetch_result` persists the etag / last-modified
/// observed after a fetch (lifted off [`StoreBackend`](crate::StoreBackend) in
/// P4 so feed lifecycle state lives on the feed seam).  The field-level CRUD
/// conveniences (`insert_feed` / `update_feed` / `set_feed_status`) were added
/// in Phase 2 so the API's Feed CRUD use-cases can run through this seam.
#[async_trait(?Send)]
pub trait FeedRepository {
    /// Insert or update a feed.  Returns the feed id.
    async fn save_feed(&self, feed: &Feed) -> Result<i64, StoreError>;

    /// Load a feed by its primary key.
    async fn find_feed(&self, id: i64) -> Result<Option<Feed>, StoreError>;

    /// Record a fetch result (etag / last-modified) after a successful fetch.
    async fn record_fetch_result(
        &self,
        feed_id: i64,
        fetched_at: i64,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<(), StoreError>;

    /// Insert a feed by URL.  Returns `None` when a feed with that URL already
    /// exists (the D1 `INSERT OR IGNORE` dedupe contract).
    async fn insert_feed(
        &self,
        url: &str,
        title: &str,
        category: &str,
        interval: i64,
    ) -> Result<Option<i64>, StoreError>;

    /// Update a feed's editable fields; only the provided fields change.
    async fn update_feed(
        &self,
        id: i64,
        title: Option<&str>,
        category: Option<&str>,
        interval: Option<i64>,
        extraction_level: Option<&str>,
    ) -> Result<(), StoreError>;

    /// Set a feed's lifecycle status (e.g. `"inactive"` to soft-delete).
    async fn set_feed_status(&self, id: i64, status: &str) -> Result<(), StoreError>;
}
