use async_trait::async_trait;

use crate::{NewSource, Source, StoreError};

/// Repository for content-source governance records.
#[async_trait(?Send)]
pub trait SourceRepository {
    /// Create or update a source entry (upsert on feed_id).
    async fn save_source(&self, source: &NewSource) -> Result<i64, StoreError>;
    /// Get source by its primary key.
    async fn find_source(&self, id: i64) -> Result<Option<Source>, StoreError>;
    /// Get source by feed_id (the most common lookup path).
    async fn find_source_by_feed(&self, feed_id: i64) -> Result<Option<Source>, StoreError>;
    /// Delete a source entry.
    async fn delete_source(&self, id: i64) -> Result<(), StoreError>;
}
