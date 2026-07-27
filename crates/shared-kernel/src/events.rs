//! Domain events + Integration events for the Sulix Cognitive Loop.
//!
//! **Domain Events** — strongly typed enums, one per bounded context.
//! Each variant carries a dedicated payload struct so the compiler catches
//! schema drift.
//!
//! **Integration Events** — JSON envelopes for cross-context communication
//! via the outbox → R2 archive pipeline.
//!
//! **OutboxPublisher trait** — infrastructure contract for reliable event
//! delivery (at-least-once via D1 `object_outbox` table).

use serde::{Deserialize, Serialize};

// ── Event ID generation ──

/// Crude event ID for MVP (no chrono dependency yet).
fn event_id() -> String {
    // js_sys::Date isn't available in shared-kernel (pure Rust).
    // The caller should inject a real ID; this is a placeholder.
    format!("evt_{}", fastrand::u64(..))
}

// ────────────────────────────────────────────
//  Decision Domain Events
// ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DecisionDomainEvent {
    Created(DecisionCreated),
    StatusChanged(DecisionStatusChanged),
    OutcomeObserved(OutcomeObserved),
    Evaluated(DecisionEvaluated),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionCreated {
    pub decision_id: String,
    pub hypothesis: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionStatusChanged {
    pub decision_id: String,
    pub old_status: String,
    pub new_status: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeObserved {
    pub decision_id: String,
    pub verdict: String,
    pub evidence_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEvaluated {
    pub decision_id: String,
    pub confidence_delta: f64,
    pub evaluator: String,
}

// ────────────────────────────────────────────
//  Signal Domain Events
// ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignalDomainEvent {
    Created(SignalCreated),
    ScoreChanged(SignalScoreChanged),
    StatusChanged(SignalStatusChanged),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalCreated {
    pub thread_id: String,
    pub entity_id: String,
    pub initial_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalScoreChanged {
    pub thread_id: String,
    pub old_score: f64,
    pub new_score: f64,
    pub trend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalStatusChanged {
    pub thread_id: String,
    pub old_status: String,
    pub new_status: String,
}

// ────────────────────────────────────────────
//  Integration Event Envelope
// ────────────────────────────────────────────

/// JSON-serializable envelope for cross-context communication
/// via outbox → R2 archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationEvent {
    pub event_id: String,
    pub source_context: String,
    pub event_type: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub payload: serde_json::Value,
    pub occurred_at: i64,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
}

impl IntegrationEvent {
    /// Build an `IntegrationEvent` from any serialisable payload.
    pub fn new(
        source_context: &str,
        aggregate_type: &str,
        aggregate_id: &str,
        event_type: &str,
        payload: &impl Serialize,
        occurred_at: i64,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            event_id: event_id(),
            source_context: source_context.to_string(),
            aggregate_type: aggregate_type.to_string(),
            aggregate_id: aggregate_id.to_string(),
            event_type: event_type.to_string(),
            payload: serde_json::to_value(payload)?,
            occurred_at,
            correlation_id: None,
            causation_id: None,
        })
    }

    /// Attach correlation / causation IDs for tracing.
    pub fn with_trace(mut self, correlation_id: String, causation_id: String) -> Self {
        self.correlation_id = Some(correlation_id);
        self.causation_id = Some(causation_id);
        self
    }

    /// Build an `IntegrationEvent` from a domain `IntelligenceEvent`.
    ///
    /// This is the canonical path for cross-context event emission — domain
    /// aggregates produce `IntelligenceEvent` variants, and the application
    /// layer calls this method to wrap them for outbox delivery.
    pub fn from_intelligence(
        event: &IntelligenceEvent,
        aggregate_id: &str,
        payload: &impl Serialize,
        occurred_at: i64,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            event_id: event_id(),
            source_context: event.aggregate_type().to_string(),
            aggregate_type: event.aggregate_type().to_string(),
            aggregate_id: aggregate_id.to_string(),
            event_type: event.event_type().to_string(),
            payload: serde_json::to_value(payload)?,
            occurred_at,
            correlation_id: None,
            causation_id: None,
        })
    }
}

// ────────────────────────────────────────────
//  Unified Intelligence Event (Sprint 6.2A)
// ────────────────────────────────────────────

/// Canonical domain event for the entire intelligence pipeline.
///
/// Every bounded context produces variants of this enum. The application
/// layer wraps them in `IntegrationEvent` and publishes via `OutboxPublisher`.
/// Downstream consumers match on variants to trigger cross-context handlers
/// (Reflection, Memory, Notification, etc.).
///
/// This is the event contract for the Intelligence OS. New domain capabilities
/// add a variant here, not a new event system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum IntelligenceEvent {
    // ── Observation Context ──
    #[serde(rename = "observation.created")]
    ObservationCreated { observation_id: String, source_type: String, title: String },

    // ── Intelligence Context ──
    #[serde(rename = "claim.created")]
    ClaimCreated { claim_id: String, article_id: i64, claim_type: String, statement: String },

    #[serde(rename = "claim.evaluated")]
    ClaimEvaluated { claim_id: String, confidence: f64 },

    #[serde(rename = "signal.detected")]
    SignalDetected { thread_id: String, signal_key: String, title: String, score: f64 },

    #[serde(rename = "signal.score_changed")]
    SignalScoreChanged { thread_id: String, old_score: f64, new_score: f64 },

    // ── Decision Context ──
    #[serde(rename = "decision.proposed")]
    DecisionProposed { decision_id: String, title: String, confidence: f64, decision_type: String },

    #[serde(rename = "decision.approved")]
    DecisionApproved { decision_id: String, approved_by: String },

    #[serde(rename = "decision.completed")]
    DecisionCompleted { decision_id: String, outcome_count: usize },

    #[serde(rename = "decision.invalidated")]
    DecisionInvalidated { decision_id: String, reason: String },

    // ── Outcome Context ──
    #[serde(rename = "outcome.recorded")]
    OutcomeRecorded { decision_id: String, metric: String, verdict: String },

    // ── Reflection Context ──
    #[serde(rename = "reflection.generated")]
    ReflectionGenerated { reflection_id: String, decision_id: String, quality_score: f64, lesson_count: usize },
}

impl IntelligenceEvent {
    /// Human-readable event type string (matches the serialised `event_type` tag).
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::ObservationCreated { .. } => "observation.created",
            Self::ClaimCreated { .. } => "claim.created",
            Self::ClaimEvaluated { .. } => "claim.evaluated",
            Self::SignalDetected { .. } => "signal.detected",
            Self::SignalScoreChanged { .. } => "signal.score_changed",
            Self::DecisionProposed { .. } => "decision.proposed",
            Self::DecisionApproved { .. } => "decision.approved",
            Self::DecisionCompleted { .. } => "decision.completed",
            Self::DecisionInvalidated { .. } => "decision.invalidated",
            Self::OutcomeRecorded { .. } => "outcome.recorded",
            Self::ReflectionGenerated { .. } => "reflection.generated",
        }
    }

    /// Aggregate-type segment for event routing and R2 key construction.
    pub fn aggregate_type(&self) -> &'static str {
        match self {
            Self::ObservationCreated { .. } => "observation",
            Self::ClaimCreated { .. } | Self::ClaimEvaluated { .. } => "claim",
            Self::SignalDetected { .. } | Self::SignalScoreChanged { .. } => "signal",
            Self::DecisionProposed { .. }
            | Self::DecisionApproved { .. }
            | Self::DecisionCompleted { .. }
            | Self::DecisionInvalidated { .. } => "decision",
            Self::OutcomeRecorded { .. } => "outcome",
            Self::ReflectionGenerated { .. } => "reflection",
        }
    }
}

// ────────────────────────────────────────────
//  Outbox Publisher (infrastructure contract)
// ────────────────────────────────────────────

/// At-least-once event delivery via the D1 outbox table.
/// Implementations live in the `infrastructure` layer.
#[async_trait::async_trait(?Send)]
pub trait OutboxPublisher {
    /// Serialise and enqueue an integration event.
    async fn publish(&self, event: &IntegrationEvent) -> Result<(), String>;
}
