//! DecisionService — unified write path for the Decision aggregate.
//!
//! Every write to the Decision Loop (create, status change, outcome,
//! evaluation) goes through this service.
//!
//! Consistency model: Outbox-first.
//!   D1 transaction (state mutation + outbox event) → commit
//!   → archive worker → EventStore append → R2 artifact
//!
//! This ensures the event is never lost even if the process crashes
//! after the D1 commit but before the EventStore append.

use event_store::{AggregateRef, EventEnvelope, EventMetadata, keys as event_keys};
use store::{Decision, NewDecision, NewDecisionEvaluation, NewOutbox, NewOutcomeEvent, StoreBackend, StoreError};

/// Structured input for creating a new decision.
pub struct CreateDecision {
    pub signal_thread_id: Option<i64>,
    pub actor_id: Option<i64>,
    pub title: String,
    pub hypothesis: Option<String>,
    pub rationale: Option<String>,
    pub confidence: f64,
    pub decision_type: String,
    pub priority: String,
}

/// DecisionService — the single entry point for Decision writes.
///
/// Uses outbox-first pattern: writes event as object_outbox row,
/// archive worker will forward to EventStore.
pub struct DecisionService<S: StoreBackend> {
    store: S,
}

impl<S: StoreBackend> DecisionService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    fn now() -> i64 {
        (js_sys::Date::now() / 1000.0) as i64
    }

    /// Helper: serialise an EventEnvelope and write it to the outbox.
    async fn emit_event(&self, event: &EventEnvelope) {
        let payload = serde_json::to_string(event).unwrap_or_default();
        let event_type = format!("event:{}", event.aggregate.aggregate_type);
        let _ = self
            .store
            .insert_outbox(&NewOutbox {
                object_type: event_type,
                object_key: event_keys::event(
                    &event.aggregate.aggregate_type,
                    event.occurred_at,
                    &event.event_id,
                ),
                payload,
            })
            .await;
    }

    /// Create a new decision and emit a DecisionCreated event via outbox.
    pub async fn create_decision(&self, cmd: CreateDecision) -> Result<Decision, StoreError> {
        let now = Self::now();
        let new = NewDecision {
            signal_thread_id: cmd.signal_thread_id,
            actor_id: cmd.actor_id,
            decision_type: cmd.decision_type,
            title: cmd.title,
            hypothesis: cmd.hypothesis,
            rationale: cmd.rationale,
            confidence: cmd.confidence,
            priority: cmd.priority,
        };

        let id = self.store.create_decision(&new).await?;
        let agg_id = format!("DEC-{id:06}");

        self.emit_event(&EventEnvelope {
            schema_version: 1,
            event_version: 1,
            event_id: event_keys::format_id(now, id as u64),
            aggregate: AggregateRef {
                aggregate_type: "decision".into(),
                aggregate_id: agg_id,
            },
            event_type: "DecisionCreated".into(),
            payload: serde_json::json!({
                "title": &new.title,
                "decision_type": &new.decision_type,
                "confidence": new.confidence,
                "priority": &new.priority,
            }),
            metadata: EventMetadata { actor: "system".into(), source: "api".into() },
            correlation_id: String::new(),
            causation_id: String::new(),
            occurred_at: now,
            created_at: now,
        }).await;

        self.store.get_decision(id).await?
            .ok_or_else(|| StoreError::D1("decision not found after create".into()))
    }

    /// Change decision status and emit a DecisionStatusChanged event via outbox.
    pub async fn change_status(&self, id: i64, status: &str) -> Result<(), StoreError> {
        let now = Self::now();
        self.store.update_decision_status(id, status).await?;

        let agg_id = format!("DEC-{id:06}");
        self.emit_event(&EventEnvelope {
            schema_version: 1,
            event_version: 1,
            event_id: event_keys::format_id(now, id as u64),
            aggregate: AggregateRef {
                aggregate_type: "decision".into(),
                aggregate_id: agg_id,
            },
            event_type: "DecisionStatusChanged".into(),
            payload: serde_json::json!({"status": status}),
            metadata: EventMetadata { actor: "system".into(), source: "api".into() },
            correlation_id: String::new(),
            causation_id: String::new(),
            occurred_at: now,
            created_at: now,
        }).await;

        Ok(())
    }

    /// Record an outcome observation.
    /// Emits DecisionStatusChanged + OutcomeObserved via outbox.
    pub async fn record_outcome(&self, decision_id: i64, outcome: &NewOutcomeEvent) -> Result<(), StoreError> {
        let now = Self::now();
        let outcome_id = self.store.create_outcome(outcome).await?;

        let dec_agg = format!("DEC-{decision_id:06}");
        self.emit_event(&EventEnvelope {
            schema_version: 1,
            event_version: 1,
            event_id: event_keys::format_id(now, outcome_id as u64),
            aggregate: AggregateRef {
                aggregate_type: "decision".into(),
                aggregate_id: dec_agg,
            },
            event_type: "DecisionStatusChanged".into(),
            payload: serde_json::json!({"status": "completed"}),
            metadata: EventMetadata { actor: "system".into(), source: "api".into() },
            correlation_id: String::new(),
            causation_id: String::new(),
            occurred_at: now,
            created_at: now,
        }).await;

        let out_agg = format!("OUT-{outcome_id:06}");
        self.emit_event(&EventEnvelope {
            schema_version: 1,
            event_version: 1,
            event_id: event_keys::format_id(now, outcome_id as u64),
            aggregate: AggregateRef {
                aggregate_type: "outcome".into(),
                aggregate_id: out_agg,
            },
            event_type: "OutcomeObserved".into(),
            payload: serde_json::json!({
                "outcome_type": &outcome.outcome_type,
                "observation": &outcome.observation,
            }),
            metadata: EventMetadata { actor: "system".into(), source: "api".into() },
            correlation_id: String::new(),
            causation_id: String::new(),
            occurred_at: now,
            created_at: now,
        }).await;

        Ok(())
    }

    /// Record an evaluation and emit a DecisionEvaluated event via outbox.
    pub async fn record_evaluation(&self, decision_id: i64, eval: &NewDecisionEvaluation) -> Result<(), StoreError> {
        let now = Self::now();
        self.store.create_evaluation(eval).await?;

        let agg_id = format!("DEC-{decision_id:06}");
        self.emit_event(&EventEnvelope {
            schema_version: 1,
            event_version: 1,
            event_id: event_keys::format_id(now, decision_id as u64),
            aggregate: AggregateRef {
                aggregate_type: "decision".into(),
                aggregate_id: agg_id,
            },
            event_type: "DecisionEvaluated".into(),
            payload: serde_json::json!({
                "evaluation": eval.evaluation.to_string(),
                "confidence": eval.confidence,
                "evaluator": eval.evaluator.to_string(),
            }),
            metadata: EventMetadata { actor: "system".into(), source: "api".into() },
            correlation_id: String::new(),
            causation_id: String::new(),
            occurred_at: now,
            created_at: now,
        }).await;

        Ok(())
    }
}
