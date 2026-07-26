use async_trait::async_trait;

use crate::{Feed, StoreError};

/// Feed aggregate persistence.
///
/// `save_feed` covers both insert and update; `find_feed` loads by primary key.
/// Feed lifecycle events (fetch results, status changes) are tracked on
/// [`super::super::backend::StoreBackend`] until they get their own domain.
#[async_trait(?Send)]
pub trait FeedRepository {
    /// Insert or update a feed.  Returns the feed id.
    async fn save_feed(&self, feed: &Feed) -> Result<i64, StoreError>;

    /// Load a feed by its primary key.
    async fn find_feed(&self, id: i64) -> Result<Option<Feed>, StoreError>;
}
