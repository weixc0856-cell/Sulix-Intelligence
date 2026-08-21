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

#[cfg(test)]
mod tests {
    use super::*;

    /// JSON-level serde round-trip: serialise → deserialise → re-serialise,
    /// and assert the two JSON documents are identical. This locks the event
    /// schema without requiring `PartialEq` on the domain types.
    fn round_trip<T>(v: &T)
    where
        T: Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
    {
        let json = serde_json::to_string(v).unwrap();
        let back: T = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_value(&back).unwrap(), serde_json::to_value(v).unwrap(), "serde round-trip mismatch");
    }

    #[test]
    fn decision_domain_events_round_trip_all_variants() {
        round_trip(&DecisionDomainEvent::Created(DecisionCreated {
            decision_id: "DEC-000001".into(),
            hypothesis: "X causes Y".into(),
            confidence: 0.8,
        }));
        round_trip(&DecisionDomainEvent::StatusChanged(DecisionStatusChanged {
            decision_id: "DEC-000001".into(),
            old_status: "draft".into(),
            new_status: "approved".into(),
            reason: "review".into(),
        }));
        round_trip(&DecisionDomainEvent::OutcomeObserved(OutcomeObserved {
            decision_id: "DEC-000001".into(),
            verdict: "confirmed".into(),
            evidence_url: Some("https://example.com".into()),
        }));
        round_trip(&DecisionDomainEvent::Evaluated(DecisionEvaluated {
            decision_id: "DEC-000001".into(),
            confidence_delta: 0.1,
            evaluator: "ai".into(),
        }));
    }

    #[test]
    fn signal_domain_events_round_trip_all_variants() {
        round_trip(&SignalDomainEvent::Created(SignalCreated {
            thread_id: "SIG-000001".into(),
            entity_id: "ENT-000001".into(),
            initial_score: 0.7,
        }));
        round_trip(&SignalDomainEvent::ScoreChanged(SignalScoreChanged {
            thread_id: "SIG-000001".into(),
            old_score: 0.5,
            new_score: 0.8,
            trend: "rising".into(),
        }));
        round_trip(&SignalDomainEvent::StatusChanged(SignalStatusChanged {
            thread_id: "SIG-000001".into(),
            old_status: "detected".into(),
            new_status: "elevated".into(),
        }));
    }

    #[test]
    fn intelligence_events_round_trip_and_serde_tag_matches_event_type() {
        let events = [
            IntelligenceEvent::ObservationCreated {
                observation_id: "OBS-000001".into(),
                source_type: "rss".into(),
                title: "t".into(),
            },
            IntelligenceEvent::ClaimCreated {
                claim_id: "C-1".into(),
                article_id: 5,
                claim_type: "prediction".into(),
                statement: "s".into(),
            },
            IntelligenceEvent::ClaimEvaluated { claim_id: "C-1".into(), confidence: 0.9 },
            IntelligenceEvent::SignalDetected {
                thread_id: "SIG-000001".into(),
                signal_key: "k".into(),
                title: "t".into(),
                score: 0.6,
            },
            IntelligenceEvent::SignalScoreChanged { thread_id: "SIG-000001".into(), old_score: 0.4, new_score: 0.7 },
            IntelligenceEvent::DecisionProposed {
                decision_id: "DEC-000001".into(),
                title: "d".into(),
                confidence: 0.8,
                decision_type: "experiment".into(),
            },
            IntelligenceEvent::DecisionApproved { decision_id: "DEC-000001".into(), approved_by: "reviewer".into() },
            IntelligenceEvent::DecisionCompleted { decision_id: "DEC-000001".into(), outcome_count: 3 },
            IntelligenceEvent::DecisionInvalidated { decision_id: "DEC-000001".into(), reason: "superseded".into() },
            IntelligenceEvent::OutcomeRecorded {
                decision_id: "DEC-000001".into(),
                metric: "accuracy".into(),
                verdict: "achieved".into(),
            },
            IntelligenceEvent::ReflectionGenerated {
                reflection_id: "REF-000001".into(),
                decision_id: "DEC-000001".into(),
                quality_score: 0.85,
                lesson_count: 2,
            },
        ];
        for ev in events {
            // The serialised `event_type` tag must equal `event_type()` — this
            // locks the serde rename ↔ code match (drift breaks cross-context routing).
            let json = serde_json::to_value(&ev).unwrap();
            assert_eq!(json["event_type"], ev.event_type(), "serde tag must match event_type()");
            let back: IntelligenceEvent = serde_json::from_value(json).unwrap();
            assert_eq!(
                serde_json::to_value(&back).unwrap(),
                serde_json::to_value(&ev).unwrap(),
                "round-trip mismatch for {}",
                ev.event_type()
            );
        }
    }

    #[test]
    fn intelligence_event_aggregate_type_routes_to_expected_contexts() {
        let obs = IntelligenceEvent::ObservationCreated {
            observation_id: "OBS-1".into(),
            source_type: "rss".into(),
            title: "t".into(),
        };
        let sig = IntelligenceEvent::SignalDetected {
            thread_id: "SIG-1".into(),
            signal_key: "k".into(),
            title: "t".into(),
            score: 0.5,
        };
        let dec = IntelligenceEvent::DecisionProposed {
            decision_id: "DEC-1".into(),
            title: "d".into(),
            confidence: 0.8,
            decision_type: "experiment".into(),
        };
        let refl = IntelligenceEvent::ReflectionGenerated {
            reflection_id: "REF-1".into(),
            decision_id: "DEC-1".into(),
            quality_score: 0.8,
            lesson_count: 1,
        };
        assert_eq!(obs.aggregate_type(), "observation");
        assert_eq!(sig.aggregate_type(), "signal");
        assert_eq!(dec.aggregate_type(), "decision");
        assert_eq!(refl.aggregate_type(), "reflection");
    }

    #[test]
    fn integration_event_new_wraps_payload_and_metadata() {
        let payload = DecisionCreated { decision_id: "DEC-000001".into(), hypothesis: "h".into(), confidence: 0.8 };
        let ev =
            IntegrationEvent::new("decision", "decision", "DEC-000001", "decision.created", &payload, 1_752_000_000)
                .unwrap();
        assert_eq!(ev.source_context, "decision");
        assert_eq!(ev.aggregate_type, "decision");
        assert_eq!(ev.aggregate_id, "DEC-000001");
        assert_eq!(ev.event_type, "decision.created");
        assert_eq!(ev.occurred_at, 1_752_000_000);
        assert!(ev.correlation_id.is_none());
        assert!(ev.causation_id.is_none());
        assert!(!ev.event_id.is_empty());
        assert_eq!(ev.payload["decision_id"], "DEC-000001");
        assert_eq!(ev.payload["hypothesis"], "h");
    }

    #[test]
    fn integration_event_with_trace_attaches_ids() {
        let payload = DecisionCreated { decision_id: "DEC-1".into(), hypothesis: "h".into(), confidence: 0.8 };
        let ev = IntegrationEvent::new("decision", "decision", "DEC-1", "decision.created", &payload, 100)
            .unwrap()
            .with_trace("corr-1".into(), "cause-1".into());
        assert_eq!(ev.correlation_id.as_deref(), Some("corr-1"));
        assert_eq!(ev.causation_id.as_deref(), Some("cause-1"));
    }

    #[test]
    fn integration_event_from_intelligence_derives_contract_fields() {
        let payload = IntelligenceEvent::DecisionProposed {
            decision_id: "DEC-000001".into(),
            title: "d".into(),
            confidence: 0.8,
            decision_type: "experiment".into(),
        };
        let ev = IntegrationEvent::from_intelligence(&payload, "DEC-000001", &payload, 100).unwrap();
        assert_eq!(ev.event_type, "decision.proposed");
        assert_eq!(ev.aggregate_type, "decision");
        assert_eq!(ev.aggregate_id, "DEC-000001");
        assert_eq!(ev.source_context, "decision");
        assert_eq!(ev.payload["title"], "d");
    }
}
