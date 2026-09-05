use async_trait::async_trait;

use crate::{NewOutbox, OutboxEntry, StoreError};

/// Object-outbox persistence (deferred R2 archive writes).
///
/// Lifted off [`StoreBackend`](crate::StoreBackend) in P4 so infra adapters
/// and event-store backends bind this instead of the legacy supertrait.
#[async_trait(?Send)]
pub trait OutboxStore {
    /// Enqueue a new outbox entry for deferred R2 archive write.
    async fn insert_outbox(&self, entry: &NewOutbox) -> Result<i64, StoreError>;

    /// Drain up to `limit` pending outbox entries, oldest first.
    async fn drain_outbox(&self, limit: u32) -> Result<Vec<OutboxEntry>, StoreError>;

    /// Mark an outbox entry as successfully archived.
    async fn mark_outbox_archived(&self, id: i64) -> Result<(), StoreError>;

    /// Mark an outbox entry as failed (retries exhausted).
    async fn mark_outbox_failed(&self, id: i64) -> Result<(), StoreError>;
}
