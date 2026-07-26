//! Events — shared message contracts for queue-driven intelligence pipeline.
//!
//! Sprint 6.1: standardized message format for INTELLIGENCE_QUEUE,
//! REFLECTION_QUEUE, and DLQ.

use serde::{Deserialize, Serialize};

/// Standard message envelope for all intelligence pipeline queues.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelligenceMessage {
    /// Machine-readable event type, e.g. "ArticleStored", "ClaimCreated".
    pub event_type: String,
    /// Entity identifier with type prefix, e.g. "article:123", "claim:456".
    pub entity_id: String,
    /// JSON-encoded event payload.
    pub payload: serde_json::Value,
    /// Retry attempt number (0 = first attempt).
    pub attempt: u32,
    /// Unix timestamp of original creation.
    pub created_at: i64,
}

impl IntelligenceMessage {
    /// Create a new message with attempt = 0.
    pub fn new(event_type: &str, entity_id: &str, payload: serde_json::Value) -> Self {
        Self {
            event_type: event_type.to_string(),
            entity_id: entity_id.to_string(),
            payload,
            attempt: 0,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
        }
    }

    /// Increment retry attempt.
    pub fn retry(&self) -> Self {
        let mut m = self.clone();
        m.attempt += 1;
        m
    }
}
