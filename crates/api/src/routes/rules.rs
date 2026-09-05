//! Rules CRUD API handlers.
//!
//! Routes:
//! - `GET    /api/rules`       — list all rules
//! - `GET    /api/rules/:id`   — get rule by id
//! - `POST   /api/rules`       — create a new rule
//! - `PUT    /api/rules/:id`   — update an existing rule
//! - `DELETE /api/rules/:id`   — delete (disable) a rule
//!
//! Orchestration (condition-fragment → full-rule JSON rewrapping, existence
//! checks) lives in [`application::RuleService`]; these handlers map its
//! outcomes onto the HTTP contract.

use application::{RuleError, RuleService};
use serde::Deserialize;
use serde_json::json;
use store::Store;
use worker::*;

use crate::shared::{params, response};

pub(crate) async fn rules_list(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = RuleService::new(ctx.data.clone());
    match service.list().await {
        Ok(list) => response::json_ok(json!({"rules": list})),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn rules_get(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = RuleService::new(ctx.data.clone());
    let id = match params::param_i64(&ctx, "id") {
        Some(v) => v,
        None => return response::json_err(400, "invalid id"),
    };
    match service.get(id).await {
        Ok(Some(rule)) => response::json_ok(json!({"rule": rule})),
        Ok(None) => response::json_err(404, "rule not found"),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

#[derive(Deserialize)]
struct CreateRuleBody {
    name: String,
    rule_json: String,
    audience_tag: Option<String>,
    signal_type: Option<String>,
    score_delta: Option<f64>,
}

pub(crate) async fn rules_create(mut req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = RuleService::new(ctx.data.clone());
    let body: CreateRuleBody = match req.json().await {
        Ok(b) => b,
        Err(_) => return response::json_err(400, "invalid JSON body"),
    };
    if body.name.is_empty() {
        return response::json_err(400, "name is required");
    }
    if body.rule_json.is_empty() {
        return response::json_err(400, "rule_json is required");
    }
    match service
        .create(
            &body.name,
            &body.rule_json,
            body.audience_tag.as_deref(),
            body.signal_type.as_deref(),
            body.score_delta,
        )
        .await
    {
        Ok(Some(id)) => match service.get(id).await {
            Ok(Some(rule)) => response::json_ok(json!({"rule": rule})),
            _ => response::json_ok(json!({"id": id})),
        },
        Ok(None) => response::json_err(500, "rule creation returned no id"),
        Err(RuleError::InvalidCondition(msg)) => response::json_err(400, &format!("invalid condition JSON: {msg}")),
        Err(RuleError::NotFound) => response::json_err(500, "rule not found"),
        Err(RuleError::Store(e)) => response::json_err_internal(&e.to_string()),
    }
}

#[derive(Deserialize)]
struct UpdateRuleBody {
    name: Option<String>,
    rule_json: Option<String>,
    enabled: Option<bool>,
    signal_type: Option<Option<String>>,
}

pub(crate) async fn rules_update(mut req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = RuleService::new(ctx.data.clone());
    let id = match params::param_i64(&ctx, "id") {
        Some(v) => v,
        None => return response::json_err(400, "invalid id"),
    };
    let body: UpdateRuleBody = match req.json().await {
        Ok(b) => b,
        Err(_) => return response::json_err(400, "invalid JSON body"),
    };
    match service
        .update(
            id,
            body.name.as_deref(),
            body.rule_json.as_deref(),
            body.enabled,
            body.signal_type.as_ref().map(|opt| opt.as_deref()),
        )
        .await
    {
        Ok(Some(rule)) => response::json_ok(json!({"rule": rule})),
        Ok(None) => response::json_err(404, "rule not found"),
        Err(RuleError::NotFound) => response::json_err(404, "rule not found for update"),
        Err(RuleError::InvalidCondition(msg)) => response::json_err(400, &format!("invalid condition JSON: {msg}")),
        Err(RuleError::Store(e)) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn rules_delete(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = RuleService::new(ctx.data.clone());
    let id = match params::param_i64(&ctx, "id") {
        Some(v) => v,
        None => return response::json_err(400, "invalid id"),
    };
    match service.delete(id).await {
        Ok(()) => response::json_ok(json!({"status": "disabled", "id": id})),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}
