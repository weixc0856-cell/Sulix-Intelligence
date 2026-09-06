//! Decision write handlers — decision-engine vertical (D2, P3).
//!
//! The four decision-engine routes now orchestrate the real application
//! use-case ([`DecisionService`]) which drives the `decision-engine` aggregate;
//! this module is the **delivery adapter**: it builds the outbox `EventEnvelope`s
//! the use-case deliberately does not know about (SD-C adapter model), preserving
//! the outbox event contract byte-for-byte:
//!
//! - event types `DecisionCreated` / `DecisionStatusChanged` / `OutcomeObserved`
//!   / `DecisionEvaluated`
//! - `object_type = "event:{aggregate_type}"`
//! - ordering = row persisted before envelope (both writes complete in the
//!   use-case before any envelope is emitted here)
//!
//! Behaviour notes (P3 decisions, 2026-09-06):
//! - create walks the aggregate to `Executing` (D1 `'active'`) so read-side
//!   `'active'` consumers keep seeing fresh decisions; a single `DecisionCreated`
//!   envelope is emitted (operation-keyed).
//! - status strings map onto lifecycle commands with aliases (`active|executing`
//!   → execute, `completed` → complete, …); unknown strings → 400.
//! - recording an outcome only advances an **Executing** decision to completed
//!   (SD-A2) — a `DecisionStatusChanged{completed}` envelope is emitted **only**
//!   when the row actually flipped, fixing the legacy fake-complete bug. The
//!   `OutcomeObserved` fact envelope is always emitted on a recorded outcome.
//! - an idempotent re-post (decision already at the target status) succeeds but
//!   writes no row and emits no envelope (no write → no event).
//!
//! Routes:
//! - `POST   /api/intelligence/signals/:id/decisions`    — create decision for signal
//! - `POST   /api/intelligence/decisions/:id/status`      — update status
//! - `POST   /api/intelligence/decisions/:id/outcomes`    — record outcome observation
//! - `POST   /api/intelligence/decisions/:id/evaluations` — record evaluation
//! - `POST   /api/decision-records`                       — create decision record
//! - `POST   /api/decision-records/:id/outcomes`          — create outcome metric

use application::{DecisionError, DecisionLifecycleCommand, DecisionService};
use composition::ProductionAppServices;
use event_store::{keys as event_keys, AggregateRef, EventEnvelope, EventMetadata};
use infrastructure::decision_repository::D1DecisionRepository;
use serde_json::{json, Value};
use store::{D1Store, DecisionRepository as _, NewOutbox, NewOutcomeEvent};
use worker::*;

use super::response;

/// Emit a single outbox envelope. Best-effort by design (SD-D): the decision /
/// fact row is already persisted by the use-case before this runs, so a failed
/// outbox insert loses the *event*, never the row.
async fn emit_envelope(store: &D1Store, event: &EventEnvelope) {
    let payload = serde_json::to_string(event).unwrap_or_default();
    let event_type = format!("event:{}", event.aggregate.aggregate_type);
    let _ = store
        .insert_outbox(&NewOutbox {
            object_type: event_type,
            object_key: event_keys::event(&event.aggregate.aggregate_type, event.occurred_at, &event.event_id),
            payload,
        })
        .await;
}

/// Build a decision/outcome outbox envelope (byte-parity with the pre-P3
/// `DecisionService::emit_event` construction).
fn envelope(
    event_type: &str,
    aggregate_type: &str,
    aggregate_id: String,
    payload: Value,
    now: i64,
    seq: u64,
) -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        event_version: 1,
        event_id: event_keys::format_id(now, seq),
        aggregate: AggregateRef { aggregate_type: aggregate_type.into(), aggregate_id },
        event_type: event_type.into(),
        payload,
        metadata: EventMetadata { actor: "system".into(), source: "api".into() },
        correlation_id: String::new(),
        causation_id: String::new(),
        occurred_at: now,
        created_at: now,
    }
}

/// Wire the real use-case over the composition-root store: the aggregate
/// repository adapter + the same store for the fact/id ports.
fn use_case(store: &D1Store) -> DecisionService<D1DecisionRepository<D1Store>, D1Store> {
    DecisionService::new(D1DecisionRepository::new(store.clone()), store.clone())
}

/// Map an HTTP `status` string onto a lifecycle command (P3 decision):
/// aliases collapse onto the canonical tokens; anything else is a 400.
fn parse_status_command(status: &str, reason: Option<&str>) -> Result<DecisionLifecycleCommand, ()> {
    match status {
        "approve" | "approved" => Ok(DecisionLifecycleCommand::Approve),
        "execute" | "executing" | "active" => Ok(DecisionLifecycleCommand::Execute),
        "complete" | "completed" => Ok(DecisionLifecycleCommand::Complete),
        "invalidate" | "invalidated" | "superseded" => Ok(DecisionLifecycleCommand::Invalidate {
            reason: reason.unwrap_or("invalidated via status endpoint").to_string(),
        }),
        _ => Err(()),
    }
}

fn now() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}

/// Map a use-case domain error onto an HTTP response.
fn map_decision_error(e: DecisionError) -> Result<Response> {
    match &e {
        DecisionError::NotFound(id) => {
            console_log!("[Sulix:decisions] not found: {id}");
            response::json_err(404, "decision not found")
        }
        DecisionError::InvalidTransition { .. } => {
            console_log!("[Sulix:decisions] invalid transition: {e}");
            response::json_err(400, "invalid status transition")
        }
        DecisionError::MissingOutcome => response::json_err(400, "decision has no observed outcome"),
        _ => {
            console_log!("[Sulix:decisions] error: {e}");
            response::json_err_internal("decision write failed")
        }
    }
}

// ── HTTP handlers (decision-engine vertical) ──────────────────────────────

/// POST /api/intelligence/signals/:id/decisions
pub(crate) async fn create(mut req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let store = ctx.data.store.clone();
    let use_case = use_case(&store);
    let signal_id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid signal thread id"),
    };

    let body: serde_json::Value = match req.json().await {
        Ok(v) => v,
        Err(_) => return response::json_err(400, "invalid request body"),
    };

    let title = match body["title"].as_str() {
        Some(t) => t.to_string(),
        None => return response::json_err(400, "title is required"),
    };

    // Expected outcomes are aggregate state (SD-B). The legacy write path had
    // no such input and persisted NULL; the new path always persists a value
    // (possibly `[]`). Parse them when the caller sends them.
    let expected_outcomes: Vec<decision_engine::ExpectedOutcome> = body["expected_outcomes"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    Some(decision_engine::ExpectedOutcome {
                        metric: item["metric"].as_str()?.to_string(),
                        expected_value: item["expected_value"].as_str()?.to_string(),
                        measurement_method: item["measurement_method"].as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let created = match use_case
        .create(decision_engine::ProposeDecision {
            id: 0, // system-assigned: the use-case allocates the row id first
            title,
            hypothesis: body["hypothesis"].as_str().map(String::from),
            confidence: body["confidence"].as_f64().unwrap_or(0.5),
            rationale: body["rationale"].as_str().map(String::from),
            decision_type: body["decision_type"].as_str().unwrap_or("monitor").to_string(),
            priority: body["priority"].as_str().unwrap_or("medium").to_string(),
            signal_thread_id: Some(signal_id),
            actor_id: body["actor_id"].as_i64().or(Some(0)),
            expected_outcomes,
        })
        .await
    {
        Ok(c) => c,
        Err(e) => return map_decision_error(e),
    };

    let decision_id = created.decision_id;
    let t = now();
    emit_envelope(
        &store,
        &envelope(
            "DecisionCreated",
            "decision",
            format!("DEC-{decision_id:06}"),
            json!({
                "title": created.aggregate.title(),
                "decision_type": created.aggregate.decision_type(),
                "confidence": created.aggregate.confidence(),
                "priority": created.aggregate.priority(),
            }),
            t,
            decision_id as u64,
        ),
    )
    .await;

    // Read the persisted row back for the response — byte-parity with the
    // legacy `{ success, decision }` shape (status `'active'`, row timestamps).
    let decision = match store.find_decision(decision_id).await {
        Ok(Some(d)) => d,
        Ok(None) => {
            console_log!("[Sulix:decisions] row missing after create");
            return response::json_err_internal("create decision failed");
        }
        Err(e) => {
            console_log!("[Sulix:decisions] read-back failed: {e}");
            return response::json_err_internal("create decision failed");
        }
    };
    response::json_ok(json!({ "success": true, "decision": decision }))
}

/// POST /api/intelligence/decisions/:id/status
pub(crate) async fn update_status(mut req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let store = ctx.data.store.clone();
    let use_case = use_case(&store);
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
    let command = match parse_status_command(status, body["reason"].as_str()) {
        Ok(c) => c,
        Err(()) => {
            console_log!("[Sulix:decisions] unknown status token: {status}");
            return response::json_err(400, "invalid status");
        }
    };

    match use_case.transition(id, command).await {
        Ok(transition) => {
            // No-op re-post (already at target): nothing was written → no event.
            if transition.transitioned {
                // Read the stored row so the event payload carries the actual
                // persisted status bucket (single source of truth).
                if let Ok(Some(row)) = store.find_decision(id).await {
                    emit_envelope(
                        &store,
                        &envelope(
                            "DecisionStatusChanged",
                            "decision",
                            format!("DEC-{id:06}"),
                            json!({ "status": row.status }),
                            now(),
                            id as u64,
                        ),
                    )
                    .await;
                }
            }
            response::json_ok(json!({ "success": true }))
        }
        Err(e) => map_decision_error(e),
    }
}

/// POST /api/intelligence/decisions/:id/outcomes
pub(crate) async fn create_outcome(mut req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let store = ctx.data.store.clone();
    let use_case = use_case(&store);
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

    match use_case.record_outcome(&outcome).await {
        Ok(recording) => {
            let t = now();
            let outcome_id = recording.outcome_id;
            // Fact envelope — always emitted for a recorded outcome.
            emit_envelope(
                &store,
                &envelope(
                    "OutcomeObserved",
                    "outcome",
                    format!("OUT-{outcome_id:06}"),
                    json!({ "outcome_type": outcome.outcome_type, "observation": outcome.observation }),
                    t,
                    outcome_id as u64,
                ),
            )
            .await;

            // Lifecycle envelope — only when the row actually flipped to
            // completed (SD-A2: an Executing decision completing on outcome).
            if recording.completed {
                emit_envelope(
                    &store,
                    &envelope(
                        "DecisionStatusChanged",
                        "decision",
                        format!("DEC-{decision_id:06}"),
                        json!({ "status": "completed" }),
                        t,
                        outcome_id as u64,
                    ),
                )
                .await;
            }
            response::json_ok(json!({ "success": true }))
        }
        Err(e) => map_decision_error(e),
    }
}

/// POST /api/intelligence/decisions/:id/evaluations
pub(crate) async fn create_evaluation(mut req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let store = ctx.data.store.clone();
    let use_case = use_case(&store);
    let decision_id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid decision id"),
    };

    let body: serde_json::Value = match req.json().await {
        Ok(v) => v,
        Err(_) => return response::json_err(400, "invalid request body"),
    };

    let eval = store::NewDecisionEvaluation {
        decision_id,
        evaluation: body["evaluation"].as_str().unwrap_or("inconclusive").into(),
        confidence: body["confidence"].as_f64(),
        reasoning: body["reasoning"].as_str().map(String::from),
        evaluator: body["evaluator"].as_str().unwrap_or("manual").into(),
        evaluated_at: body["evaluated_at"].as_i64(),
    };

    match use_case.record_evaluation(&eval).await {
        Ok(_) => {
            emit_envelope(
                &store,
                &envelope(
                    "DecisionEvaluated",
                    "decision",
                    format!("DEC-{decision_id:06}"),
                    json!({
                        "evaluation": eval.evaluation.to_string(),
                        "confidence": eval.confidence,
                        "evaluator": eval.evaluator.to_string(),
                    }),
                    now(),
                    decision_id as u64,
                ),
            )
            .await;
            response::json_ok(json!({ "success": true }))
        }
        Err(e) => map_decision_error(e),
    }
}

// ── Sprint 6.0: Decision Record writes (unchanged — not part of the
// ── decision-engine vertical) ─────────────────────────────────────────────

/// POST /api/decision-records — create a decision record
pub(crate) async fn create_decision_record(
    mut req: Request,
    ctx: RouteContext<ProductionAppServices>,
) -> Result<Response> {
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
    let store = ctx.data.store.clone();
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
pub(crate) async fn create_outcome_metric(
    mut req: Request,
    ctx: RouteContext<ProductionAppServices>,
) -> Result<Response> {
    #[derive(serde::Deserialize)]
    struct OutcomeInput {
        metric: String,
        expected_value: Option<String>,
        measurement_method: Option<String>,
    }
    let store = ctx.data.store.clone();
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid id"),
    };
    let input: OutcomeInput = match req.json().await {
        Ok(b) => b,
        Err(_) => return response::json_err(400, "invalid request body"),
    };
    let body = store::NewOutcome {
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
