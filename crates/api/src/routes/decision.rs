//! Decision Loop route handlers.
//!
//! Routes:
//! - `GET    /api/intelligence/decisions`               — list decisions
//! - `GET    /api/intelligence/decisions/:id`            — decision detail
//! - `POST   /api/intelligence/signals/:id/decisions`   — create decision for signal
//! - `GET    /api/intelligence/signals/:id/decisions`    — decisions by signal
//! - `POST   /api/intelligence/decisions/:id/status`     — update status

use serde_json::json;
use store::{D1Store, NewDecisionEvaluation, NewOutcomeEvent, Store};
use worker::*;

use crate::services::decision::{CreateDecision, DecisionService};
use crate::shared::response;

fn build_decision_service(env: &Env) -> Result<DecisionService<D1Store>> {
    Ok(DecisionService::new(D1Store::new(env.d1("DB")?)))
}

/// GET /api/intelligence/decisions?status=active
pub async fn list(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let status = req.url().ok().and_then(|u| u.query_pairs().find(|(k, _)| k == "status").map(|(_, v)| v.to_string()));
    let limit = 50u32;
    match store.list_decisions(status.as_deref(), limit).await {
        Ok(decisions) => response::json_ok(json!({ "success": true, "decisions": decisions })),
        Err(e) => {
            console_log!("[Sulix:decisions] list failed: {e}");
            response::json_err_internal("list decisions failed")
        }
    }
}

/// GET /api/intelligence/decisions/:id
pub async fn detail(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid decision id"),
    };
    match store.get_decision(id).await {
        Ok(Some(decision)) => response::json_ok(json!({ "success": true, "decision": decision })),
        Ok(None) => response::json_err(404, "decision not found"),
        Err(e) => {
            console_log!("[Sulix:decisions] get failed: {e}");
            response::json_err_internal("get decision failed")
        }
    }
}

/// POST /api/intelligence/signals/:id/decisions
pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let svc = match build_decision_service(&ctx.env) {
        Ok(s) => s,
        Err(_) => return response::json_err(503, "service unavailable"),
    };
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

/// GET /api/intelligence/signals/:id/decisions
pub async fn by_signal(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let signal_id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid signal thread id"),
    };
    match store.decisions_by_signal(signal_id).await {
        Ok(decisions) => response::json_ok(json!({ "success": true, "decisions": decisions })),
        Err(e) => {
            console_log!("[Sulix:decisions] by_signal failed: {e}");
            response::json_err_internal("query failed")
        }
    }
}

/// POST /api/intelligence/decisions/:id/status
pub async fn update_status(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let svc = match build_decision_service(&ctx.env) {
        Ok(s) => s,
        Err(_) => return response::json_err(503, "service unavailable"),
    };
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

/// GET /api/intelligence/decisions/stats — Decision Accuracy Dashboard.
pub async fn stats(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    match store.decision_stats().await {
        Ok(stats) => response::json_ok(json!(stats)),
        Err(e) => {
            console_log!("[Sulix:decisions] stats failed: {e}");
            response::json_err_internal("decision stats query failed")
        }
    }
}

// ===== Outcome Event handlers =====

/// POST /api/intelligence/decisions/:id/outcomes
pub async fn create_outcome(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let svc = match build_decision_service(&ctx.env) {
        Ok(s) => s,
        Err(_) => return response::json_err(503, "service unavailable"),
    };
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

// ===== Decision Evaluation handlers =====

/// POST /api/intelligence/decisions/:id/evaluations
pub async fn create_evaluation(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let svc = match build_decision_service(&ctx.env) {
        Ok(s) => s,
        Err(_) => return response::json_err(503, "service unavailable"),
    };
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

/// GET /api/intelligence/decisions/:id/evaluations
pub async fn list_evaluations(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let decision_id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid decision id"),
    };
    match store.get_decision_evaluations(decision_id).await {
        Ok(evaluations) => response::json_ok(json!({ "success": true, "evaluations": evaluations })),
        Err(e) => {
            console_log!("[Sulix:evaluations] list failed: {e}");
            response::json_err_internal("list evaluations failed")
        }
    }
}

/// GET /api/intelligence/decisions/:id/outcomes
pub async fn list_outcomes(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let decision_id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid decision id"),
    };
    match store.get_decision_outcomes(decision_id).await {
        Ok(outcomes) => response::json_ok(json!({ "success": true, "outcomes": outcomes })),
        Err(e) => {
            console_log!("[Sulix:outcomes] list failed: {e}");
            response::json_err_internal("list outcomes failed")
        }
    }
}

// ── Decision Timeline ──

#[derive(serde::Serialize)]
struct TimelineEvent {
    pub timestamp: i64,
    pub event_type: String,
    pub title: String,
    pub description: String,
}

/// GET /api/intelligence/decisions/:id/timeline
pub async fn timeline(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid decision id"),
    };

    let decision = match store.get_decision(id).await {
        Ok(Some(d)) => d,
        Ok(None) => return response::json_err(404, "decision not found"),
        Err(e) => {
            console_log!("[Sulix:timeline] {e}");
            return response::json_err_internal("timeline failed");
        }
    };

    let mut events: Vec<TimelineEvent> = Vec::new();

    events.push(TimelineEvent {
        timestamp: decision.created_at,
        event_type: "decision.created".into(),
        title: "Decision registered".into(),
        description: format!("Status: {}, Confidence: {:.0}%", decision.status, decision.confidence * 100.0),
    });

    if let Ok(outcomes) = store.get_decision_outcomes(id).await {
        for o in &outcomes {
            events.push(TimelineEvent {
                timestamp: o.observed_at,
                event_type: "outcome.observed".into(),
                title: format!("Outcome: {}", o.outcome_type),
                description: o.observation.clone(),
            });
        }
    }

    if let Ok(evals) = store.get_decision_evaluations(id).await {
        for e in &evals {
            events.push(TimelineEvent {
                timestamp: e.evaluated_at,
                event_type: "decision.evaluated".into(),
                title: format!("Judgment: {}", e.evaluation),
                description: e.reasoning.clone().unwrap_or_default(),
            });
        }
    }

    if let Ok(Some(r)) = store.get_reflection_by_decision(id).await {
        if let Some(started) = r.started_at {
            events.push(TimelineEvent {
                timestamp: started,
                event_type: "reflection.generated".into(),
                title: "AI Reflection".into(),
                description: r.result.unwrap_or_default(),
            });
        }
    }

    events.sort_by_key(|e| e.timestamp);

    let learning = store.get_reflection_by_decision(id).await.ok().flatten().and_then(|r| r.result);

    response::json_ok(serde_json::json!({ "success": true, "events": events, "learning": learning }))
}

// ── Decision Explanation ──

#[derive(serde::Serialize)]
struct SupportingEvidence {
    title: String,
    strength: f64,
    source: Option<String>,
}

#[derive(serde::Serialize)]
struct ConfidenceDriver {
    factor: String,
    impact: String,
}

#[derive(serde::Serialize)]
struct FrameworkTrace {
    id: String,
    name: String,
    category: String,
    relevance: f64,
    reasoning: String,
}

#[derive(serde::Serialize)]
struct ExplanationResponse {
    decision_id: String,
    decision_title: String,
    hypothesis: Option<String>,
    confidence: f64,
    trend: String,
    supporting_evidence: Vec<SupportingEvidence>,
    confidence_drivers: Vec<ConfidenceDriver>,
    uncertainties: Vec<String>,
    outcome_summary: Option<String>,
    frameworks_applied: Vec<FrameworkTrace>,
}

/// GET /api/intelligence/decisions/:id/explanation
///
/// Returns a structured explanation of why the system holds this belief:
/// supporting evidence, confidence drivers, and known uncertainties.
/// This is the core "Why Sulix Thinks This" API.
pub async fn explanation(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid decision id"),
    };

    let decision = match store.get_decision(id).await {
        Ok(Some(d)) => d,
        Ok(None) => return response::json_err(404, "decision not found"),
        Err(e) => {
            console_log!("[Sulix:explanation] get_decision failed: {e}");
            return response::json_err_internal("explanation query failed");
        }
    };

    // Determine trend from confidence history
    let trend = match store.list_confidence_history("decision", &id.to_string()).await {
        Ok(events) if events.len() >= 2 => {
            let latest = events.last().unwrap();
            let prev = &events[events.len() - 2];
            if latest.confidence > prev.previous_confidence.unwrap_or(0.0) {
                "increasing".into()
            } else if latest.confidence < prev.confidence {
                "decreasing".into()
            } else {
                "stable".into()
            }
        }
        _ => "stable".into(),
    };

    // Load evidence from signal thread
    let mut supporting_evidence: Vec<SupportingEvidence> = Vec::new();
    if let Some(signal_id) = decision.signal_thread_id {
        if let Ok(Some(detail)) = store.load_signal_detail(signal_id).await {
            for article in &detail.evidence_top {
                supporting_evidence.push(SupportingEvidence {
                    title: article.title.clone(),
                    strength: article.score.clamp(0.0, 1.0),
                    source: article.feed_name.clone(),
                });
            }
        }
    }

    // Load outcomes for accuracy summary
    let outcome_summary = match store.get_decision_outcomes(id).await {
        Ok(outcomes) if !outcomes.is_empty() => {
            let confirmed = outcomes
                .iter()
                .filter(|o| o.outcome_type == "confirmed" || o.outcome_type == "prediction_correct")
                .count();
            Some(format!("{}/{} outcomes confirmed", confirmed, outcomes.len()))
        }
        _ => None,
    };

    // Build confidence drivers
    let mut confidence_drivers: Vec<ConfidenceDriver> = Vec::new();
    let evidence_count = supporting_evidence.len();
    if evidence_count >= 3 {
        confidence_drivers.push(ConfidenceDriver {
            factor: "evidence".into(),
            impact: format!("{} independent sources", evidence_count),
        });
    } else if evidence_count >= 1 {
        confidence_drivers
            .push(ConfidenceDriver { factor: "evidence".into(), impact: format!("{} source", evidence_count) });
    }
    if trend == "increasing" {
        confidence_drivers
            .push(ConfidenceDriver { factor: "trend".into(), impact: "Confidence rising over time".into() });
    }
    if let Some(ref _hypothesis) = decision.hypothesis {
        confidence_drivers.push(ConfidenceDriver {
            factor: "analysis".into(),
            impact: "Based on structured hypothesis testing".into(),
        });
    }

    // Uncertainties
    let mut uncertainties: Vec<String> = Vec::new();
    if evidence_count < 5 {
        uncertainties.push("Limited number of supporting sources".into());
    }
    if outcome_summary.is_none() {
        uncertainties.push("Outcome not yet observed — prediction pending".into());
    }

    // Load reasoning framework traces
    let frameworks_applied: Vec<FrameworkTrace> = store
        .get_decision_framework_traces(id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| FrameworkTrace {
            id: row["framework_id"].as_str().unwrap_or("").to_string(),
            name: row["name"].as_str().unwrap_or("").to_string(),
            category: row["category"].as_str().unwrap_or("").to_string(),
            relevance: row["relevance"].as_f64().unwrap_or(0.0),
            reasoning: row["reasoning"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    let response = ExplanationResponse {
        decision_id: format!("DEC-{:06}", id),
        decision_title: decision.title,
        hypothesis: decision.hypothesis,
        confidence: decision.confidence,
        trend,
        supporting_evidence,
        confidence_drivers,
        uncertainties,
        outcome_summary,
        frameworks_applied,
    };

    response::json_ok(json!(response))
}

// ── Sprint 6.0: Decision Records ──

/// GET /api/decision-records — list decision records (?status=)
pub async fn list_decision_records(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let status = req.url().ok().and_then(|u| u.query_pairs().find(|(k, _)| k == "status").map(|(_, v)| v.to_string()));
    let limit = 50u32;
    match store.list_decision_records(status.as_deref(), limit).await {
        Ok(records) => response::json_ok(json!({ "records": records })),
        Err(e) => {
            console_log!("[Sulix:decision-records] list failed: {e}");
            response::json_err_internal("list failed")
        }
    }
}

/// POST /api/decision-records — create a decision record
pub async fn create_decision_record(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
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
    let store = Store::new(ctx.env.d1("DB")?);
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

/// GET /api/decision-records/:id — detail with memo
pub async fn get_decision_record(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid id"),
    };
    let record = match store.get_decision_record(id).await {
        Ok(Some(r)) => r,
        Ok(None) => return response::json_err(404, "not found"),
        Err(e) => {
            console_log!("[Sulix:decision-records] get failed: {e}");
            return response::json_err_internal("get failed");
        }
    };
    let outcomes = store.list_decision_outcomes(id).await.unwrap_or_default();
    let claims = store.get_decision_claims(id).await.unwrap_or_default();
    response::json_ok(json!({ "record": record, "outcomes": outcomes, "claims": claims }))
}

/// GET /api/decision-records/:id/memo — get or generate the decision memo
pub async fn decision_memo(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid id"),
    };
    let record = match store.get_decision_record(id).await {
        Ok(Some(r)) => r,
        Ok(None) => return response::json_err(404, "not found"),
        Err(e) => {
            console_log!("[Sulix:decision-records] memo get failed: {e}");
            return response::json_err_internal("memo failed");
        }
    };
    // Return existing memo or generate new one
    if let Some(ref memo_json) = record.memo_json {
        if let Ok(memo) = serde_json::from_str::<serde_json::Value>(memo_json) {
            return response::json_ok(json!({ "memo": memo }));
        }
    }
    // Load framework traces for memo sections 5+8
    let frameworks: Vec<decision_engine::FrameworkMemoSection> = store
        .get_decision_framework_traces(id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| decision_engine::FrameworkMemoSection {
            name: row["name"].as_str().unwrap_or("").to_string(),
            category: row["category"].as_str().unwrap_or("").to_string(),
            reasoning: row["reasoning"].as_str().unwrap_or("").to_string(),
        })
        .collect();
    let fw_opt: Option<&[decision_engine::FrameworkMemoSection]> =
        if frameworks.is_empty() { None } else { Some(&frameworks) };

    let memo = decision_engine::generate_memo(
        id,
        &record.title,
        &record.context,
        &record.rationale,
        record.confidence,
        None,
        fw_opt,
    );
    let memo_json = serde_json::to_string(&memo).unwrap_or_default();
    let _ = store.set_decision_memo(id, &memo_json).await;
    response::json_ok(json!({ "memo": memo }))
}

/// POST /api/decision-records/:id/outcomes — create an outcome metric
pub async fn create_outcome_metric(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    #[derive(serde::Deserialize)]
    struct OutcomeInput {
        metric: String,
        expected_value: Option<String>,
        measurement_method: Option<String>,
    }
    let store = Store::new(ctx.env.d1("DB")?);
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

/// GET /api/decision-records/:id/outcomes — list outcomes
pub async fn list_outcome_metrics(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid id"),
    };
    match store.list_decision_outcomes(id).await {
        Ok(outcomes) => response::json_ok(json!({ "outcomes": outcomes })),
        Err(e) => {
            console_log!("[Sulix:decision-records] outcomes list failed: {e}");
            response::json_err_internal("list outcomes failed")
        }
    }
}
