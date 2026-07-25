//! Event Store — unified abstraction for append-only event streams.
//!
//! Every state mutation in the Intelligence pipeline (signal events, decision
//! outcomes, reflections) produces an `Event` that is durably recorded via
//! the [`EventStore`] trait.  Events are written outbox-first to D1, then
//! asynchronously archived to R2.
//!
//! ## Architecture
//!
//! ```text
//! SignalEngine / DecisionEngine / ...
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
mod r2_backend;

use async_trait::async_trait;

pub use d1_backend::D1EventBackend;
pub use r2_backend::EventR2Backend;

/// Unique identifier for an event, formatted as `evt_{timestamp}_{aggregate_id}_{seq}`.
pub type EventId = String;

/// An immutable event in the Memory Event Stream.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Event {
    pub schema_version: i32,
    pub event_id: EventId,
    pub aggregate_type: String,
    pub aggregate_id: i64,
    pub event_type: String,
    pub payload: serde_json::Value,
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
    async fn append_event(&self, event: &Event) -> Result<EventId, EventStoreError>;

    /// Load events for an aggregate, newest first.
    async fn load_events(
        &self,
        aggregate_type: &str,
        aggregate_id: i64,
        limit: u32,
    ) -> Result<Vec<Event>, EventStoreError>;
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

    /// Helper: event_id from timestamp + aggregate_id + seq.
    pub fn format_id(aggregate_id: i64, created_at: i64, seq: u64) -> String {
        format!("evt_{created_at}_{aggregate_id}_{seq}")
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
        let id = keys::format_id(42, 1721865600, 1);
        assert_eq!(id, "evt_1721865600_42_1");
    }
}
