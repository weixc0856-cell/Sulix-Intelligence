//! EventStore-backed [`EventLog`] adapter — bridges the shared-kernel
//! append-only port onto the production event-store (D1 outbox + index,
//! async R2 archive).
//!
//! Lives in infrastructure so neither shared-kernel nor the domain crates
//! need to know about event-store.

use async_trait::async_trait;
use event_store::{keys, AggregateRef, EventEnvelope, EventMetadata, EventStore};
use shared_kernel::event_log::{DomainEvent, EventLog, EventLogError};

/// Adapts a [`event_store::EventStore`] implementation to the domain [`EventLog`].
///
/// The adapter injects storage metadata that [`DomainEvent`] intentionally does
/// not carry: envelope versioning, a generated `event_id`, empty `causation_id`,
/// actor/source provenance, and `created_at`.
pub struct EventStoreLog {
    inner: Box<dyn EventStore>,
}

impl EventStoreLog {
    pub fn new(inner: Box<dyn EventStore>) -> Self {
        Self { inner }
    }
}

/// Stable per-aggregate sequence seed, so distinct aggregates written in the
/// same second still produce distinct event ids (the D1 event index keys on
/// `event_id`). Deterministic — re-appending the same logical event yields the
/// same id.
fn seq_for_aggregate(aggregate_id: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in aggregate_id.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[async_trait(?Send)]
impl EventLog for EventStoreLog {
    async fn append(&self, event: &DomainEvent) -> Result<(), EventLogError> {
        let occurred_at = event.occurred_at;
        let envelope = EventEnvelope {
            schema_version: 1,
            event_version: 1,
            event_id: keys::format_id(occurred_at, seq_for_aggregate(&event.aggregate_id)),
            correlation_id: event.correlation_id.clone(),
            causation_id: String::new(),
            aggregate: AggregateRef {
                aggregate_type: event.aggregate_type.clone(),
                aggregate_id: event.aggregate_id.clone(),
            },
            event_type: event.event_type.clone(),
            payload: event.payload.clone(),
            metadata: EventMetadata { actor: "system".into(), source: "reflection_engine".into() },
            occurred_at,
            created_at: occurred_at,
        };
        self.inner.append_event(&envelope).await.map(|_| ()).map_err(|e| EventLogError::Persistence(e.to_string()))
    }
}
