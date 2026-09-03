//! Event Log — minimal append-only port for durable domain events.
//!
//! Cross-context events (today: reflection results) are written to the memory
//! event stream. Consumers of the event should depend on this small port rather
//! than on a specific event-store implementation (R2/D1 backend, a message
//! bus, …). The adapter — which lives in `crates/infrastructure` — supplies all
//! storage metadata (envelope ids, actor/source provenance, timestamps).
//!
//! See [`crate::events`] for the strongly-typed per-context domain events; this
//! module is the *durable record* seam for events that must survive as an audit
//! trail (event sourcing / replay), not the in-process message type.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A domain event recorded to the event log — the domain-only projection.
///
/// Deliberately minimal: carries the fields the domain can speak to. Storage
/// metadata (`schema_version`, envelope `event_id`, `causation_id`, actor/source
/// provenance, `created_at`) is injected by the infrastructure adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEvent {
    /// Stable event type, e.g. `"ReflectionGenerated"`.
    pub event_type: String,
    /// Aggregate kind the event belongs to, e.g. `"reflection"`.
    pub aggregate_type: String,
    /// Aggregate id the event belongs to, e.g. `"REF-000001"`.
    pub aggregate_id: String,
    /// Event payload (domain-specific).
    pub payload: serde_json::Value,
    /// When the event occurred (unix seconds).
    pub occurred_at: i64,
    /// Correlation id tracing a business transaction across aggregates.
    pub correlation_id: String,
}

/// Errors from [`EventLog::append`].
#[derive(Debug, thiserror::Error)]
pub enum EventLogError {
    #[error("event-log persistence: {0}")]
    Persistence(String),
}

/// Append-only event log port.
///
/// Append-only for now: no load/query/replay is declared until a consumer needs
/// it (avoid modelling the read side of a store we don't yet read from).
#[async_trait(?Send)]
pub trait EventLog {
    /// Append an event to the log.
    async fn append(&self, event: &DomainEvent) -> Result<(), EventLogError>;
}

// Blanket impl: Box<T> implements EventLog when T does (mirrors the EventStore
// pattern) so composition roots can hold `Box<dyn EventLog>`.
#[async_trait(?Send)]
impl<T: EventLog + ?Sized> EventLog for Box<T> {
    async fn append(&self, event: &DomainEvent) -> Result<(), EventLogError> {
        (**self).append(event).await
    }
}
