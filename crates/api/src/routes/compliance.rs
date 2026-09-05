//! Compliance API — takedown request workflow and audit trail.
//!
//! Takedown follows a state machine: submitted → reviewing → approved/rejected.
//! Approved takedowns create content_visibility_overrides (block serving).
//! Source policy is NOT modified by takedown — overrides are independent.

use composition::ProductionAppServices;
use serde::Deserialize;
use serde_json::json;
use worker::*;

use crate::shared::response;

#[derive(Deserialize)]
struct SubmitTakedownBody {
    source_id: Option<i64>,
    article_id: Option<i64>,
    requester_email: String,
    reason: String,
}

#[derive(Deserialize)]
struct TakedownStatusBody {
    status: String,
    notes: Option<String>,
}

/// POST /api/compliance/takedown
/// Submit a takedown request. If article_id is provided, immediately
/// blocks article serving via visibility override.
pub async fn submit_takedown(mut req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let service = &ctx.data.compliance;
    let body: SubmitTakedownBody = match req.json().await {
        Ok(b) => b,
        Err(_) => return response::json_err(400, "invalid request body"),
    };

    if body.requester_email.is_empty() || body.reason.is_empty() {
        return response::json_err(400, "requester_email and reason are required");
    }
    if body.source_id.is_none() && body.article_id.is_none() {
        return response::json_err(400, "either source_id or article_id is required");
    }

    match service.submit(body.source_id, body.article_id, &body.requester_email, &body.reason).await {
        Ok(takedown_id) => response::json_ok(json!({
            "takedown_id": takedown_id,
            "status": "submitted",
            "message": "Takedown request submitted. Content access has been blocked."
        })),
        Err(e) => {
            console_log!("[Sulix:compliance] create takedown failed: {e}");
            response::json_err_internal("create takedown failed")
        }
    }
}

/// GET /api/compliance/takedowns
/// List all takedown requests (admin).
pub async fn list_takedowns(req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let service = &ctx.data.compliance;

    let status_filter =
        req.url().ok().and_then(|u| u.query_pairs().find(|(k, _)| k == "status").map(|(_, v)| v.to_string()));
    let limit = req
        .url()
        .ok()
        .and_then(|u| u.query_pairs().find(|(k, _)| k == "limit").and_then(|(_, v)| v.parse::<u32>().ok()))
        .unwrap_or(50);

    match service.list(status_filter.as_deref(), limit).await {
        Ok(takedowns) => response::json_ok(json!({ "takedowns": takedowns })),
        Err(e) => {
            console_log!("[Sulix:compliance] list takedowns failed: {e}");
            response::json_err_internal("list takedowns failed")
        }
    }
}

/// PUT /api/compliance/takedowns/:id/status
/// Update takedown status (admin).
pub async fn update_takedown_status(mut req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let service = &ctx.data.compliance;
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid takedown id"),
    };

    let body: TakedownStatusBody = match req.json().await {
        Ok(b) => b,
        Err(_) => return response::json_err(400, "invalid request body"),
    };

    match body.status.as_str() {
        "approved" | "rejected" | "reviewing" => {}
        _ => return response::json_err(400, "invalid status: must be 'approved', 'rejected', or 'reviewing'"),
    }

    match service.update_status(id, &body.status, body.notes.as_deref()).await {
        Ok(_) => response::json_ok(json!({ "status": "updated", "takedown_id": id })),
        Err(e) => {
            console_log!("[Sulix:compliance] update status failed: {e}");
            response::json_err_internal("update takedown status failed")
        }
    }
}
