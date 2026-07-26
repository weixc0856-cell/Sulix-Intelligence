//! Legacy D1-only EventStore — reads from the `signal_events` table.
//!
//! Used as a read fallback during the transition period while the R2 archive
//! is being backfilled.  Write path: appends to `signal_events` table.

use async_trait::async_trait;
use store::StoreBackend;

use crate::{EventEnvelope, EventId, EventStore, EventStoreError};

/// Legacy EventStore that reads/writes the D1 `signal_events` table.
///
/// This exists solely for backward compatibility during the Sprint 5.2
/// transition.  Once all signal events have been migrated to R2, this
/// backend can be removed.
pub struct D1EventBackend<S: StoreBackend> {
    pub store: S,
}

impl<S: StoreBackend> D1EventBackend<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

#[async_trait(?Send)]
impl<S: StoreBackend + 'static> EventStore for D1EventBackend<S> {
    async fn append_event(&self, event: &EventEnvelope) -> Result<EventId, EventStoreError> {
        let payload_str =
            serde_json::to_string(&event.payload).map_err(|e| EventStoreError::Serialisation(e.to_string()))?;

        let numeric_id: i64 = event
            .aggregate
            .aggregate_id
            .trim_start_matches("SIG-")
            .trim_start_matches("DEC-")
            .trim_start_matches("OUT-")
            .parse()
            .unwrap_or(0);

        self.store.insert_signal_event(numeric_id, &event.event_type, Some(&payload_str)).await?;

        Ok(event.event_id.clone())
    }

    async fn load_events(
        &self,
        _aggregate_type: &str,
        aggregate_id: &str,
        limit: u32,
    ) -> Result<Vec<EventEnvelope>, EventStoreError> {
        let numeric_id: i64 = aggregate_id
            .trim_start_matches("SIG-")
            .trim_start_matches("DEC-")
            .trim_start_matches("OUT-")
            .parse()
            .unwrap_or(0);

        let rows = self.store.load_signal_events(numeric_id, limit).await?;

        let events: Vec<EventEnvelope> = rows
            .into_iter()
            .map(|r| EventEnvelope {
                schema_version: 1,
                event_version: 1,
                event_id: format!("legacy_{}", r.id),
                aggregate: crate::AggregateRef {
                    aggregate_type: "signal_thread".into(),
                    aggregate_id: format!("SIG-{}", r.thread_id),
                },
                event_type: r.event_type,
                payload: r.payload.and_then(|p| serde_json::from_str(&p).ok()).unwrap_or(serde_json::Value::Null),
                metadata: crate::EventMetadata { actor: "system".into(), source: "legacy".into() },
                correlation_id: String::new(),
                causation_id: String::new(),
                occurred_at: r.created_at,
                created_at: r.created_at,
            })
            .collect();

        Ok(events)
    }
}
