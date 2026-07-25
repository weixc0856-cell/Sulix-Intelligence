//! NoopEventStore — safe default / test-double that discards all events.
//!
//! Used in contexts where the EventStore is optional (dev, test, degraded
//! mode).  Replaces `Option<&dyn EventStore>` — the calling code always
//! has a valid EventStore reference, never needs to branch on `None`.

use async_trait::async_trait;

use crate::{EventEnvelope, EventId, EventStore, EventStoreError};

/// No-op EventStore that silently discards events and returns empty reads.
///
/// Implements the Null Object pattern so producers never need to handle
/// `Option<&dyn EventStore>`.
pub struct NoopEventStore;

impl NoopEventStore {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoopEventStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait(?Send)]
impl EventStore for NoopEventStore {
    async fn append_event(&self, _event: &EventEnvelope) -> Result<EventId, EventStoreError> {
        Ok("noop".into())
    }

    async fn load_events(
        &self,
        _aggregate_type: &str,
        _aggregate_id: &str,
        _limit: u32,
    ) -> Result<Vec<EventEnvelope>, EventStoreError> {
        Ok(Vec::new())
    }
}
