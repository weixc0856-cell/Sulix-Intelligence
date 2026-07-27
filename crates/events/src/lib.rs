//! Events — shared message contracts for queue-driven intelligence pipeline.
//!
//! Sprint 6.2E: Standardized event envelope wrapping `IntelligenceEvent` for
//! Cloudflare Queue transit. Each event carries enough context for the
//! consumer to route and process without a D1 lookup.
//!
//! ## Queue topology
//!
//! Single `INTELLIGENCE_QUEUE` with typed event routing via `event_type`:
//!
//! - `observation.*` → observation handlers
//! - `claim.*` → claim extraction handlers
//! - `signal.*` → signal detection handlers
//! - `decision.*` → decision/outcome handlers
//! - `reflection.*` → reflection handlers

use serde::{Deserialize, Serialize};
use shared_kernel::events::IntelligenceEvent;

/// Standard message envelope for event-driven intelligence queues.
///
/// Wraps an [`IntelligenceEvent`] with transport metadata (attempt count,
/// creation timestamp) so queue consumers can handle retries and ordering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// Machine-readable event type (e.g. "decision.proposed").
    pub event_type: String,
    /// Entity identifier with type prefix (e.g. "DEC-000042").
    pub entity_id: String,
    /// JSON-encoded IntelligenceEvent payload.
    pub payload: serde_json::Value,
    /// Retry attempt number (0 = first attempt).
    pub attempt: u32,
    /// Unix timestamp of original creation.
    pub created_at: i64,
}

impl EventEnvelope {
    /// Create a new envelope from an [`IntelligenceEvent`].
    pub fn from_event(event: &IntelligenceEvent) -> Self {
        let entity_id = match event {
            IntelligenceEvent::ObservationCreated { observation_id, .. } => observation_id.clone(),
            IntelligenceEvent::ClaimCreated { claim_id, .. } => claim_id.clone(),
            IntelligenceEvent::ClaimEvaluated { claim_id, .. } => claim_id.clone(),
            IntelligenceEvent::SignalDetected { thread_id, .. } => thread_id.clone(),
            IntelligenceEvent::SignalScoreChanged { thread_id, .. } => thread_id.clone(),
            IntelligenceEvent::DecisionProposed { decision_id, .. } => decision_id.clone(),
            IntelligenceEvent::DecisionApproved { decision_id, .. } => decision_id.clone(),
            IntelligenceEvent::DecisionCompleted { decision_id, .. } => decision_id.clone(),
            IntelligenceEvent::DecisionInvalidated { decision_id, .. } => decision_id.clone(),
            IntelligenceEvent::OutcomeRecorded { decision_id, .. } => decision_id.clone(),
            IntelligenceEvent::ReflectionGenerated { reflection_id, .. } => reflection_id.clone(),
        };

        Self {
            event_type: event.event_type().to_string(),
            entity_id,
            payload: serde_json::to_value(event).unwrap_or_default(),
            attempt: 0,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
        }
    }

    /// Increment retry attempt for DLQ handling.
    pub fn retry(&self) -> Self {
        let mut m = self.clone();
        m.attempt += 1;
        m
    }
}
