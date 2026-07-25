//! Event Store — unified abstraction for append-only event streams.
//!
//! Every state mutation in the Intelligence pipeline (signal events, decision
//! outcomes, reflections) produces an [`EventEnvelope`] that is durably recorded
//! via the [`EventStore`] trait.  Events are written outbox-first to D1, then
//! asynchronously archived to R2.
//!
//! ## Architecture
//!
//! ```text
//! SignalEngine / DecisionService / ...
//!     │
//!     └── EventStore::append_event()
//!             │
//!             ├── D1 outbox (durable, ordered)
//!             ├── D1 event_archive_index (metadata)
//!             └── archive worker → R2 (eventually consistent)
//! ```
//!
//! Reading follows the reverse path: D1 index → R2 payload → legacy fallback.

mod d1_backend;
mod noop;
mod r2_backend;

use async_trait::async_trait;

pub use d1_backend::D1EventBackend;
pub use noop::NoopEventStore;
pub use r2_backend::EventR2Backend;

/// Unique identifier for an event, formatted as `evt_{timestamp}_{aggregate_id}_{seq}`.
pub type EventId = String;

/// Reference to an aggregate (entity) in the system.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AggregateRef {
    /// e.g. "decision", "signal_thread", "outcome", "reflection"
    pub aggregate_type: String,
    /// e.g. "DEC-000123", "SIG-042", "OUT-000001"
    pub aggregate_id: String,
}

/// Helper for constructing typed aggregate IDs consistently.
pub struct AggregateId;

impl AggregateId {
    pub fn decision(id: i64) -> String { format!("DEC-{id:06}") }
    pub fn signal(id: i64) -> String { format!("SIG-{id:06}") }
    pub fn outcome(id: i64) -> String { format!("OUT-{id:06}") }
    pub fn reflection(id: i64) -> String { format!("REF-{id:06}") }
    pub fn memory(id: i64) -> String { format!("MEM-{id:06}") }
}

/// Provenance metadata for an event — who caused it and how.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EventMetadata {
    /// e.g. "user", "system", "agent"
    pub actor: String,
    /// e.g. "api", "cron", "worker", "import"
    pub source: String,
}

/// Default event_version for deserializing legacy events.
fn default_event_version() -> i32 { 1 }

/// An immutable event in the Memory Event Stream.
///
/// This is the canonical envelope for all events in the Sulix Intelligence
/// Memory Layer.  Every aggregate type (decision, outcome, signal thread,
/// reflection) uses the same envelope structure.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EventEnvelope {
    pub schema_version: i32,
    pub event_id: EventId,
    #[serde(default = "default_event_version")]
    pub event_version: i32,            // per-event-type version for schema evolution
    pub aggregate: AggregateRef,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub metadata: EventMetadata,
    pub occurred_at: i64,
    pub created_at: i64,
}

/// Errors from EventStore operations.
#[derive(Debug, thiserror::Error)]
pub enum EventStoreError {
    #[error("store error: {0}")]
    Store(String),
    #[error("serialisation error: {0}")]
    Serialisation(String),
}

impl From<store::StoreError> for EventStoreError {
    fn from(e: store::StoreError) -> Self {
        EventStoreError::Store(e.to_string())
    }
}

impl From<object_store::ObjectStoreError> for EventStoreError {
    fn from(e: object_store::ObjectStoreError) -> Self {
        EventStoreError::Store(e.to_string())
    }
}

/// Abstraction over the Memory Event Stream — append-only, eventually consistent.
///
/// Producers call [`append_event`] to durably record an event.  The write is
/// outbox-first: D1 outbox + index immediately, R2 archive asynchronously.
/// Consumers call [`load_events`] to read the event stream for an aggregate.
#[async_trait(?Send)]
pub trait EventStore {
    /// Append an event to the archive.  Outbox-first: the event is durable
    /// in D1 immediately; R2 archival happens asynchronously.
    async fn append_event(&self, event: &EventEnvelope) -> Result<EventId, EventStoreError>;

    /// Load events for an aggregate, newest first.
    async fn load_events(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
        limit: u32,
    ) -> Result<Vec<EventEnvelope>, EventStoreError>;
}

// Blanket impl: Box<T> implements EventStore when T does.
#[async_trait(?Send)]
impl<T: EventStore + ?Sized> EventStore for Box<T> {
    async fn append_event(&self, event: &EventEnvelope) -> Result<EventId, EventStoreError> {
        (**self).append_event(event).await
    }
    async fn load_events(&self, aggregate_type: &str, aggregate_id: &str, limit: u32) -> Result<Vec<EventEnvelope>, EventStoreError> {
        (**self).load_events(aggregate_type, aggregate_id, limit).await
    }
}

/// Key prefix helpers for the Memory Event Stream.
pub mod keys {
    /// Decompose a Unix timestamp into (year, month, day) UTC.
    fn decompose_ts(unix_secs: i64) -> (i32, u32, u32) {
        // Approximate: a year is 365.25 days; this is good enough for key paths.
        let secs_per_day: i64 = 86400;
        let days = unix_secs.div_euclid(secs_per_day);
        let year_base: i64 = 1970;
        let mut year = year_base;
        let mut remaining = days;
        loop {
            let days_in_year = if is_leap(year) { 366 } else { 365 };
            if remaining < days_in_year {
                break;
            }
            remaining -= days_in_year;
            year += 1;
        }
        let month_days = if is_leap(year) {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };
        let mut month: u32 = 0;
        for (i, &md) in month_days.iter().enumerate() {
            if remaining < md {
                month = (i + 1) as u32;
                break;
            }
            remaining -= md;
        }
        (year as i32, month, (remaining + 1) as u32)
    }

    fn is_leap(year: i64) -> bool {
        (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
    }

    /// R2 object key for an event.
    ///
    /// Format: `memory/events/{aggregate_type}/{year}/{month}/{day}/{event_id}.json`
    pub fn event(aggregate_type: &str, occurred_at: i64, event_id: &str) -> String {
        let (year, month, day) = decompose_ts(occurred_at);
        format!("memory/events/{aggregate_type}/{year:04}/{month:02}/{day:02}/{event_id}.json")
    }

    /// Helper: event_id from created_at + seq.
    pub fn format_id(created_at: i64, seq: u64) -> String {
        format!("evt_{created_at}_{seq}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_event_format() {
        // Using fixed timestamp: 2024-07-25 = 1721865600
        let k = keys::event("signal_thread", 1721865600, "evt_1721865600_42_1");
        assert_eq!(k, "memory/events/signal_thread/2024/07/25/evt_1721865600_42_1.json");
    }

    #[test]
    fn keys_event_id_format() {
        let id = keys::format_id(1721865600, 1);
        assert_eq!(id, "evt_1721865600_1");
    }
}
