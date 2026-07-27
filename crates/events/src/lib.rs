//! Events — shared message contracts for queue-driven intelligence pipeline.
//!
//! Sprint 6.2E: Standardized event envelope wrapping `IntelligenceEvent` for
//! Cloudflare Queue transit. Each event carries enough context for the
//! consumer to route and process without a D1 lookup.

use serde::{Deserialize, Serialize};
use shared_kernel::events::IntelligenceEvent;

/// Standard message envelope for event-driven intelligence queues.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub event_type: String,
    pub entity_id: String,
    pub payload: serde_json::Value,
    pub attempt: u32,
    pub created_at: i64,
}

impl EventEnvelope {
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

    pub fn retry(&self) -> Self {
        let mut m = self.clone();
        m.attempt += 1;
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared_kernel::events::IntelligenceEvent;

    #[test]
    fn envelope_from_decision_proposed_event() {
        let event = IntelligenceEvent::DecisionProposed {
            decision_id: "DEC-000042".into(),
            title: "Test decision".into(),
            confidence: 0.85,
            decision_type: "experiment".into(),
        };
        let envelope = EventEnvelope::from_event(&event);
        assert_eq!(envelope.event_type, "decision.proposed");
        assert_eq!(envelope.entity_id, "DEC-000042");
        assert_eq!(envelope.attempt, 0);
    }

    #[test]
    fn envelope_from_claim_created_event() {
        let event = IntelligenceEvent::ClaimCreated {
            claim_id: "CLM-000001".into(),
            article_id: 42,
            claim_type: "trend".into(),
            statement: "AI adoption grows".into(),
        };
        let envelope = EventEnvelope::from_event(&event);
        assert_eq!(envelope.event_type, "claim.created");
        assert_eq!(envelope.entity_id, "CLM-000001");
    }

    #[test]
    fn retry_increments_attempt() {
        let event = IntelligenceEvent::ObservationCreated {
            observation_id: "OBS-000001".into(),
            source_type: "RssFeed".into(),
            title: "Test".into(),
        };
        let envelope = EventEnvelope::from_event(&event).retry();
        assert_eq!(envelope.attempt, 1);
        let envelope = envelope.retry();
        assert_eq!(envelope.attempt, 2);
    }

    #[test]
    fn envelope_serde_roundtrip() {
        let event = IntelligenceEvent::ReflectionGenerated {
            reflection_id: "REF-000001".into(),
            decision_id: "DEC-000001".into(),
            quality_score: 0.85,
            lesson_count: 3,
        };
        let envelope = EventEnvelope::from_event(&event);
        let json = serde_json::to_string(&envelope).unwrap();
        let parsed: EventEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type, "reflection.generated");
        assert_eq!(parsed.entity_id, "REF-000001");
        assert_eq!(parsed.attempt, 0);
    }

    #[test]
    fn all_event_types_have_entity_id() {
        let events: Vec<IntelligenceEvent> = vec![
            IntelligenceEvent::ObservationCreated {
                observation_id: "OBS-1".into(), source_type: "RssFeed".into(), title: "T".into(),
            },
            IntelligenceEvent::ClaimCreated {
                claim_id: "CLM-1".into(), article_id: 1, claim_type: "fact".into(), statement: "S".into(),
            },
            IntelligenceEvent::ClaimEvaluated { claim_id: "CLM-1".into(), confidence: 0.8 },
            IntelligenceEvent::SignalDetected {
                thread_id: "SIG-1".into(), signal_key: "k".into(), title: "T".into(), score: 0.5,
            },
            IntelligenceEvent::SignalScoreChanged {
                thread_id: "SIG-1".into(), old_score: 0.5, new_score: 0.6,
            },
            IntelligenceEvent::DecisionProposed {
                decision_id: "DEC-1".into(), title: "T".into(), confidence: 0.8, decision_type: "e".into(),
            },
            IntelligenceEvent::DecisionApproved { decision_id: "DEC-1".into(), approved_by: "user".into() },
            IntelligenceEvent::DecisionCompleted { decision_id: "DEC-1".into(), outcome_count: 2 },
            IntelligenceEvent::DecisionInvalidated { decision_id: "DEC-1".into(), reason: "superseded".into() },
            IntelligenceEvent::OutcomeRecorded {
                decision_id: "DEC-1".into(), metric: "accuracy".into(), verdict: "achieved".into(),
            },
            IntelligenceEvent::ReflectionGenerated {
                reflection_id: "REF-1".into(), decision_id: "DEC-1".into(), quality_score: 0.8, lesson_count: 2,
            },
        ];
        for event in &events {
            let envelope = EventEnvelope::from_event(event);
            assert!(!envelope.entity_id.is_empty(), "entity_id empty for {}", envelope.event_type);
        }
    }
}
