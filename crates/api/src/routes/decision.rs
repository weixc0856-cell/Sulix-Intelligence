//! Decision Loop route handlers.
//!
//! Routes:
//! - `GET    /api/intelligence/decisions`               — list decisions
//! - `GET    /api/intelligence/decisions/:id`            — decision detail
//! - `POST   /api/intelligence/signals/:id/decisions`   — create decision for signal
//! - `GET    /api/intelligence/signals/:id/decisions`    — decisions by signal
//! - `POST   /api/intelligence/decisions/:id/status`     — update status

use serde_json::json;
use worker::*;

use store::{NewDecision, NewOutcomeEvent, Store};

use crate::shared::response;

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
    let store = Store::new(ctx.env.d1("DB")?);
    let signal_id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid signal thread id"),
    };

    let body: serde_json::Value = match req.json().await {
        Ok(v) => v,
        Err(_) => return response::json_err(400, "invalid request body"),
    };

    let decision_type = body["decision_type"].as_str().unwrap_or("monitor");
    let title = match body["title"].as_str() {
        Some(t) => t,
        None => return response::json_err(400, "title is required"),
    };

    let new_decision = NewDecision {
        signal_thread_id: Some(signal_id),
        actor_id: None,
        decision_type: decision_type.to_string(),
        title: title.to_string(),
        hypothesis: body["hypothesis"].as_str().map(String::from),
        rationale: body["rationale"].as_str().map(String::from),
        confidence: body["confidence"].as_f64().unwrap_or(0.5),
        priority: body["priority"].as_str().unwrap_or("medium").to_string(),
    };

    match store.create_decision(&new_decision).await {
        Ok(id) => response::json_ok(json!({ "success": true, "id": id })),
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
    let store = Store::new(ctx.env.d1("DB")?);
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

    match store.update_decision_status(id, status).await {
        Ok(()) => response::json_ok(json!({ "success": true })),
        Err(e) => {
            console_log!("[Sulix:decisions] update_status failed: {e}");
            response::json_err_internal("update status failed")
        }
    }
}

// ===== Outcome Event handlers =====

/// POST /api/intelligence/decisions/:id/outcomes
pub async fn create_outcome(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let decision_id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid decision id"),
    };

    let body: serde_json::Value = match req.json().await {
        Ok(v) => v,
        Err(_) => return response::json_err(400, "invalid request body"),
    };

    let new_outcome = NewOutcomeEvent {
        decision_id,
        outcome_type: body["outcome_type"].as_str().unwrap_or("observation").to_string(),
        observation: body["observation"].as_str().unwrap_or("").to_string(),
        evidence_url: body["evidence_url"].as_str().map(String::from),
        observed_at: body["observed_at"].as_i64(),
    };

    match store.create_outcome(&new_outcome).await {
        Ok(id) => response::json_ok(json!({ "success": true, "id": id })),
        Err(e) => {
            console_log!("[Sulix:outcomes] create failed: {e}");
            response::json_err_internal("create outcome failed")
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
