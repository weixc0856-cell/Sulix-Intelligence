//! EventStore-backed [`SignalEventLog`] adapter — bridges the signal-engine
//! event-log port onto the production event-store (D1 outbox + index, async R2
//! archive).
//!
//! Lives in infrastructure so signal-engine never depends on event-store.

use async_trait::async_trait;
use event_store::{keys, AggregateRef, EventEnvelope, EventMetadata, EventStore};
use signal_engine::error::SignalError;
use signal_engine::ports::{SignalEvent, SignalEventLog};

/// Adapts a [`event_store::EventStore`] implementation to the domain [`SignalEventLog`].
///
/// The adapter injects storage metadata that [`SignalEvent`] intentionally does
/// not carry: envelope versioning, the derived `event_id` (from
/// `(occurred_at, sequence)` — mirroring the legacy `evt_{ts}_{seq}` scheme),
/// actor/source provenance, and empty correlation/causation ids.
pub struct EventStoreSignalLog {
    inner: Box<dyn EventStore>,
}

impl EventStoreSignalLog {
    pub fn new(inner: Box<dyn EventStore>) -> Self {
        Self { inner }
    }
}

#[async_trait(?Send)]
impl SignalEventLog for EventStoreSignalLog {
    async fn append(&self, event: &SignalEvent, sequence: u64) -> Result<(), SignalError> {
        let occurred_at = event.occurred_at;
        let envelope = EventEnvelope {
            schema_version: 1,
            event_version: 1,
            event_id: keys::format_id(occurred_at, sequence),
            correlation_id: String::new(),
            causation_id: String::new(),
            aggregate: AggregateRef {
                aggregate_type: "signal_thread".into(),
                aggregate_id: event.aggregate_id.clone(),
            },
            event_type: event.event_type.clone(),
            payload: event.payload.clone(),
            metadata: EventMetadata { actor: "system".into(), source: "cron".into() },
            occurred_at,
            created_at: occurred_at,
        };
        self.inner.append_event(&envelope).await.map(|_| ()).map_err(|e| SignalError::EventLog(e.to_string()))
    }

    async fn load(&self, aggregate_id: &str, limit: u32) -> Result<Vec<SignalEvent>, SignalError> {
        let events = self
            .inner
            .load_events("signal_thread", aggregate_id, limit)
            .await
            .map_err(|e| SignalError::EventLog(e.to_string()))?;
        Ok(events
            .into_iter()
            .map(|e| SignalEvent {
                event_type: e.event_type,
                aggregate_id: e.aggregate.aggregate_id,
                payload: e.payload,
                occurred_at: e.occurred_at,
            })
            .collect())
    }
}
