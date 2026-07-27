//! Decision domain events — produced by `DecisionAggregate`, consumed by
//! application services for outbox emission.
//!
//! These are the **domain-language** events. They get wrapped in an
//! `IntegrationEvent` (from shared-kernel) for cross-context transport.

use serde::{Deserialize, Serialize};

use crate::status::DecisionStatus;

/// Events emitted by the Decision aggregate.
///
/// Every variant carries enough context for downstream consumers
/// (Reflection, Memory, Notification) without needing to query the
/// decision store.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum DecisionDomainEvent {
    #[serde(rename = "decision.proposed")]
    Proposed { decision_id: String, title: String, confidence: f64, decision_type: String },

    #[serde(rename = "decision.approved")]
    Approved { decision_id: String, approved_by: String },

    #[serde(rename = "decision.status_changed")]
    StatusChanged { decision_id: String, from: DecisionStatus, to: DecisionStatus },

    #[serde(rename = "decision.outcome_attached")]
    OutcomeAttached { decision_id: String, metric: String, verdict: String },

    #[serde(rename = "decision.completed")]
    Completed { decision_id: String, outcome_count: usize },

    #[serde(rename = "decision.invalidated")]
    Invalidated { decision_id: String, reason: String },
}
