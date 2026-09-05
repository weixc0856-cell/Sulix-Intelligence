use async_trait::async_trait;

use crate::{Feed, StoreError};

/// Feed aggregate persistence.
///
/// `save_feed` covers both insert and update; `find_feed` loads by primary key;
/// `record_fetch_result` persists the etag / last-modified observed after a
/// fetch (lifted off [`StoreBackend`](crate::StoreBackend) in P4 so feed
/// lifecycle state lives on the feed seam).
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
}
