//! Decision write handlers + DecisionService — composition-root owned.
//!
//! Migrated from `api` in Phase 2 (Checkpoint F). The Decision write vertical
//! is GATED: this is a mechanical relocation only. The four `StoreBackend`
//! write methods (`create_decision`, `update_decision_status`,
//! `create_outcome`, `create_evaluation`) are unchanged, as is the outbox-first
//! consistency model. No domain redesign here.
//!
//! Routes:
//! - `POST   /api/intelligence/signals/:id/decisions`    — create decision for signal
//! - `POST   /api/intelligence/decisions/:id/status`      — update status
//! - `POST   /api/intelligence/decisions/:id/outcomes`    — record outcome observation
//! - `POST   /api/intelligence/decisions/:id/evaluations` — record evaluation
//! - `POST   /api/decision-records`                       — create decision record
//! - `POST   /api/decision-records/:id/outcomes`          — create outcome metric

use event_store::{keys as event_keys, AggregateRef, EventEnvelope, EventMetadata};
use serde_json::json;
use store::{
    Decision, NewDecision, NewDecisionEvaluation, NewOutbox, NewOutcomeEvent, Store, StoreBackend, StoreError,
};
use worker::*;

use super::response;

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
                object_key: event_keys::event(&event.aggregate.aggregate_type, event.occurred_at, &event.event_id),
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
            aggregate: AggregateRef { aggregate_type: "decision".into(), aggregate_id: agg_id },
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
        })
        .await;

        self.store.find_decision(id).await?.ok_or_else(|| StoreError::D1("decision not found after create".into()))
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
            aggregate: AggregateRef { aggregate_type: "decision".into(), aggregate_id: agg_id },
            event_type: "DecisionStatusChanged".into(),
            payload: serde_json::json!({"status": status}),
            metadata: EventMetadata { actor: "system".into(), source: "api".into() },
            correlation_id: String::new(),
            causation_id: String::new(),
            occurred_at: now,
            created_at: now,
        })
        .await;

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
            aggregate: AggregateRef { aggregate_type: "decision".into(), aggregate_id: dec_agg },
            event_type: "DecisionStatusChanged".into(),
            payload: serde_json::json!({"status": "completed"}),
            metadata: EventMetadata { actor: "system".into(), source: "api".into() },
            correlation_id: String::new(),
            causation_id: String::new(),
            occurred_at: now,
            created_at: now,
        })
        .await;

        let out_agg = format!("OUT-{outcome_id:06}");
        self.emit_event(&EventEnvelope {
            schema_version: 1,
            event_version: 1,
            event_id: event_keys::format_id(now, outcome_id as u64),
            aggregate: AggregateRef { aggregate_type: "outcome".into(), aggregate_id: out_agg },
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
        })
        .await;

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
            aggregate: AggregateRef { aggregate_type: "decision".into(), aggregate_id: agg_id },
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
        })
        .await;

        Ok(())
    }
}

// ── HTTP handlers ─────────────────────────────────────────────────────────

/// POST /api/intelligence/signals/:id/decisions
pub(crate) async fn create(mut req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let svc = DecisionService::new(ctx.data.clone());
    let signal_id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid signal thread id"),
    };

    let body: serde_json::Value = match req.json().await {
        Ok(v) => v,
        Err(_) => return response::json_err(400, "invalid request body"),
    };

    let title = match body["title"].as_str() {
        Some(t) => t,
        None => return response::json_err(400, "title is required"),
    };

    let cmd = CreateDecision {
        signal_thread_id: Some(signal_id),
        actor_id: body["actor_id"].as_i64().or(Some(0)),
        decision_type: body["decision_type"].as_str().unwrap_or("monitor").to_string(),
        title: title.to_string(),
        hypothesis: body["hypothesis"].as_str().map(String::from),
        rationale: body["rationale"].as_str().map(String::from),
        confidence: body["confidence"].as_f64().unwrap_or(0.5),
        priority: body["priority"].as_str().unwrap_or("medium").to_string(),
    };

    match svc.create_decision(cmd).await {
        Ok(decision) => response::json_ok(json!({ "success": true, "decision": decision })),
        Err(e) => {
            console_log!("[Sulix:decisions] create failed: {e}");
            response::json_err_internal("create decision failed")
        }
    }
}

/// POST /api/intelligence/decisions/:id/status
pub(crate) async fn update_status(mut req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let svc = DecisionService::new(ctx.data.clone());
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid decision id"),
    };

    let body: serde_json::Value = match req.json().await {
        Ok(v) => v,
        Err(_) => return response::json_err(400, "invalid request body"),
    };

    let status = match body["status"].as_str() {
        Some(s) => s,
        None => return response::json_err(400, "status is required"),
    };

    match svc.change_status(id, status).await {
        Ok(()) => response::json_ok(json!({ "success": true })),
        Err(e) => {
            console_log!("[Sulix:decisions] update_status failed: {e}");
            response::json_err_internal("update status failed")
        }
    }
}

/// POST /api/intelligence/decisions/:id/outcomes
pub(crate) async fn create_outcome(mut req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let svc = DecisionService::new(ctx.data.clone());
    let decision_id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid decision id"),
    };

    let body: serde_json::Value = match req.json().await {
        Ok(v) => v,
        Err(_) => return response::json_err(400, "invalid request body"),
    };

    let outcome = NewOutcomeEvent {
        decision_id,
        outcome_type: body["outcome_type"].as_str().unwrap_or("observation").to_string(),
        observation: body["observation"].as_str().unwrap_or("").to_string(),
        evidence_url: body["evidence_url"].as_str().map(String::from),
        observed_at: body["observed_at"].as_i64(),
    };

    match svc.record_outcome(decision_id, &outcome).await {
        Ok(()) => response::json_ok(json!({ "success": true })),
        Err(e) => {
            console_log!("[Sulix:outcomes] create failed: {e}");
            response::json_err_internal("create outcome failed")
        }
    }
}

/// POST /api/intelligence/decisions/:id/evaluations
pub(crate) async fn create_evaluation(mut req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let svc = DecisionService::new(ctx.data.clone());
    let decision_id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid decision id"),
    };

    let body: serde_json::Value = match req.json().await {
        Ok(v) => v,
        Err(_) => return response::json_err(400, "invalid request body"),
    };

    let eval = NewDecisionEvaluation {
        decision_id,
        evaluation: body["evaluation"].as_str().unwrap_or("inconclusive").into(),
        confidence: body["confidence"].as_f64(),
        reasoning: body["reasoning"].as_str().map(String::from),
        evaluator: body["evaluator"].as_str().unwrap_or("manual").into(),
        evaluated_at: body["evaluated_at"].as_i64(),
    };

    match svc.record_evaluation(decision_id, &eval).await {
        Ok(()) => response::json_ok(json!({ "success": true })),
        Err(e) => {
            console_log!("[Sulix:evaluations] create failed: {e}");
            response::json_err_internal("create evaluation failed")
        }
    }
}

// ── Sprint 6.0: Decision Record writes ──

/// POST /api/decision-records — create a decision record
pub(crate) async fn create_decision_record(mut req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    #[derive(serde::Deserialize)]
    struct CreateInput {
        title: String,
        context: Option<String>,
        decision_type: Option<String>,
        action: Option<String>,
        rationale: Option<String>,
        confidence: Option<f64>,
        signal_id: Option<i64>,
    }
    let store = ctx.data.clone();
    let input: CreateInput = match req.json().await {
        Ok(b) => b,
        Err(_) => return response::json_err(400, "invalid request body"),
    };
    let body = store::NewDecisionRecord {
        title: input.title,
        context: input.context,
        decision_type: input.decision_type,
        action: input.action,
        rationale: input.rationale,
        confidence: input.confidence.unwrap_or(0.5),
        signal_id: input.signal_id,
    };
    match store.create_decision_record(&body).await {
        Ok(id) => response::json_ok(json!({ "id": id })),
        Err(e) => {
            console_log!("[Sulix:decision-records] create failed: {e}");
            response::json_err_internal("create failed")
        }
    }
}

/// POST /api/decision-records/:id/outcomes — create an outcome metric
pub(crate) async fn create_outcome_metric(mut req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    #[derive(serde::Deserialize)]
    struct OutcomeInput {
        metric: String,
        expected_value: Option<String>,
        measurement_method: Option<String>,
    }
    let store = ctx.data.clone();
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid id"),
    };
    let input: OutcomeInput = match req.json().await {
        Ok(b) => b,
        Err(_) => return response::json_err(400, "invalid request body"),
    };
    use store::domain::decision::record_crud::NewOutcome;
    let body = NewOutcome {
        decision_id: id,
        metric: input.metric,
        expected_value: input.expected_value,
        measurement_method: input.measurement_method,
    };
    match store.create_outcome_metric(&body).await {
        Ok(outcome_id) => response::json_ok(json!({ "outcome_id": outcome_id })),
        Err(e) => {
            console_log!("[Sulix:decision-records] outcome create failed: {e}");
            response::json_err_internal("create outcome failed")
        }
    }
}
