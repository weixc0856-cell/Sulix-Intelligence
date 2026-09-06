use async_trait::async_trait;

use crate::{EventIndexEntry, StoreError};

/// Event archive index persistence (outbox-first Event Sourcing).
///
/// Infra adapters and event-store backends bind this narrow seam directly.
#[async_trait(?Send)]
pub trait EventIndexStore {
    /// Insert a row into the event_archive_index table.
    async fn insert_event_index(
        &self,
        event_id: &str,
        aggregate_type: &str,
        aggregate_id: &str,
        event_type: &str,
        object_key: &str,
        occurred_at: i64,
    ) -> Result<(), StoreError>;

    /// Find event index entries for an aggregate, newest first.
    async fn find_event_keys(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
        limit: u32,
    ) -> Result<Vec<EventIndexEntry>, StoreError>;
}
