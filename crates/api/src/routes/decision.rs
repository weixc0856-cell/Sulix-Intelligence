//! Decision Loop read route handlers.
//!
//! Decision *writes* were relocated to `worker-entry/routes/decision_write.rs`
//! in Phase 2 (Checkpoint F) — they orchestrate infrastructure (outbox-first
//! event emission) so they belong to the composition root. Read handlers here
//! delegate to `application::DecisionReadService`.
//!
//! Routes:
//! - `GET    /api/intelligence/decisions`               — list decisions
//! - `GET    /api/intelligence/decisions/:id`            — decision detail
//! - `GET    /api/intelligence/signals/:id/decisions`    — decisions by signal
//! - `GET    /api/intelligence/decisions/stats`          — decision stats
//! - `GET    /api/intelligence/decisions/:id/evaluations` — list evaluations
//! - `GET    /api/intelligence/decisions/:id/outcomes`   — list outcomes
//! - `GET    /api/intelligence/decisions/:id/timeline`   — decision timeline
//! - `GET    /api/intelligence/decisions/:id/explanation` — decision explanation

use application::DecisionReadService;
use serde_json::json;
use store::Store;
use worker::*;

use crate::shared::response;

/// GET /api/intelligence/decisions?status=active
pub async fn list(req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = DecisionReadService::new(ctx.data.clone());
    let status = req.url().ok().and_then(|u| u.query_pairs().find(|(k, _)| k == "status").map(|(_, v)| v.to_string()));
    let limit = 50u32;
    match service.list(status.as_deref(), limit).await {
        Ok(decisions) => response::json_ok(json!({ "success": true, "decisions": decisions })),
        Err(e) => {
            console_log!("[Sulix:decisions] list failed: {e}");
            response::json_err_internal("list decisions failed")
        }
    }
}

/// GET /api/intelligence/decisions/:id
pub async fn detail(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = DecisionReadService::new(ctx.data.clone());
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid decision id"),
    };
    match service.detail(id).await {
        Ok(Some(decision)) => response::json_ok(json!({ "success": true, "decision": decision })),
        Ok(None) => response::json_err(404, "decision not found"),
        Err(e) => {
            console_log!("[Sulix:decisions] get failed: {e}");
            response::json_err_internal("get decision failed")
        }
    }
}

/// GET /api/intelligence/signals/:id/decisions
pub async fn by_signal(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = DecisionReadService::new(ctx.data.clone());
    let signal_id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid signal thread id"),
    };
    match service.by_signal(signal_id).await {
        Ok(decisions) => response::json_ok(json!({ "success": true, "decisions": decisions })),
        Err(e) => {
            console_log!("[Sulix:decisions] by_signal failed: {e}");
            response::json_err_internal("query failed")
        }
    }
}

/// GET /api/intelligence/decisions/stats — Decision Accuracy Dashboard.
pub async fn stats(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = DecisionReadService::new(ctx.data.clone());
    match service.stats().await {
        Ok(stats) => response::json_ok(json!(stats)),
        Err(e) => {
            console_log!("[Sulix:decisions] stats failed: {e}");
            response::json_err_internal("decision stats query failed")
        }
    }
}

/// GET /api/intelligence/decisions/:id/evaluations
pub async fn list_evaluations(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = DecisionReadService::new(ctx.data.clone());
    let decision_id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid decision id"),
    };
    match service.list_evaluations(decision_id).await {
        Ok(evaluations) => response::json_ok(json!({ "success": true, "evaluations": evaluations })),
        Err(e) => {
            console_log!("[Sulix:evaluations] list failed: {e}");
            response::json_err_internal("list evaluations failed")
        }
    }
}

/// GET /api/intelligence/decisions/:id/outcomes
pub async fn list_outcomes(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = DecisionReadService::new(ctx.data.clone());
    let decision_id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid decision id"),
    };
    match service.list_outcomes(decision_id).await {
        Ok(outcomes) => response::json_ok(json!({ "success": true, "outcomes": outcomes })),
        Err(e) => {
            console_log!("[Sulix:outcomes] list failed: {e}");
            response::json_err_internal("list outcomes failed")
        }
    }
}

// ── Decision Timeline ──

/// GET /api/intelligence/decisions/:id/timeline
pub async fn timeline(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = DecisionReadService::new(ctx.data.clone());
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid decision id"),
    };

    match service.timeline(id).await {
        Ok(Some(timeline)) => {
            response::json_ok(json!({ "success": true, "events": timeline.events, "learning": timeline.learning }))
        }
        Ok(None) => response::json_err(404, "decision not found"),
        Err(e) => {
            console_log!("[Sulix:timeline] {e}");
            response::json_err_internal("timeline failed")
        }
    }
}

// ── Decision Explanation ──

/// GET /api/intelligence/decisions/:id/explanation
///
/// Returns a structured explanation of why the system holds this belief:
/// supporting evidence, confidence drivers, and known uncertainties.
/// This is the core "Why Sulix Thinks This" API.
pub async fn explanation(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = DecisionReadService::new(ctx.data.clone());
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid decision id"),
    };

    match service.explanation(id).await {
        Ok(Some(explanation)) => response::json_ok(json!(explanation)),
        Ok(None) => response::json_err(404, "decision not found"),
        Err(e) => {
            console_log!("[Sulix:explanation] {e}");
            response::json_err_internal("explanation query failed")
        }
    }
}

// ── Sprint 6.0: Decision Records (reads) ──

/// GET /api/decision-records — list decision records (?status=)
pub async fn list_decision_records(req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = DecisionReadService::new(ctx.data.clone());
    let status = req.url().ok().and_then(|u| u.query_pairs().find(|(k, _)| k == "status").map(|(_, v)| v.to_string()));
    let limit = 50u32;
    match service.list_records(status.as_deref(), limit).await {
        Ok(records) => response::json_ok(json!({ "records": records })),
        Err(e) => {
            console_log!("[Sulix:decision-records] list failed: {e}");
            response::json_err_internal("list failed")
        }
    }
}

/// GET /api/decision-records/:id — detail with memo
pub async fn get_decision_record(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = DecisionReadService::new(ctx.data.clone());
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid id"),
    };
    match service.record_detail(id).await {
        Ok(Some(detail)) => {
            response::json_ok(json!({ "record": detail.record, "outcomes": detail.outcomes, "claims": detail.claims }))
        }
        Ok(None) => response::json_err(404, "not found"),
        Err(e) => {
            console_log!("[Sulix:decision-records] get failed: {e}");
            response::json_err_internal("get failed")
        }
    }
}

/// GET /api/decision-records/:id/memo — get or generate the decision memo
pub async fn decision_memo(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = DecisionReadService::new(ctx.data.clone());
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid id"),
    };
    let record = match service.record(id).await {
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
    let frameworks: Vec<decision_engine::FrameworkMemoSection> = service
        .framework_traces(id)
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
    let _ = service.save_memo(id, &memo_json).await;
    response::json_ok(json!({ "memo": memo }))
}

/// GET /api/decision-records/:id/outcomes — list outcomes
pub async fn list_outcome_metrics(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = DecisionReadService::new(ctx.data.clone());
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid id"),
    };
    match service.record_outcomes(id).await {
        Ok(outcomes) => response::json_ok(json!({ "outcomes": outcomes })),
        Err(e) => {
            console_log!("[Sulix:decision-records] outcomes list failed: {e}");
            response::json_err_internal("list outcomes failed")
        }
    }
}
