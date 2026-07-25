//! Rules CRUD API handlers.
//!
//! Routes:
//! - `GET    /api/rules`       — list all rules
//! - `GET    /api/rules/:id`   — get rule by id
//! - `POST   /api/rules`       — create a new rule
//! - `PUT    /api/rules/:id`   — update an existing rule
//! - `DELETE /api/rules/:id`   — delete (disable) a rule

use crate::shared::{params, response};
use serde::Deserialize;
use store::Store;
use worker::*;

pub(crate) async fn rules_list(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    match store.list_rules().await {
        Ok(list) => response::json_ok(serde_json::json!({"rules": list})),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn rules_get(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match params::param_i64(&ctx, "id") {
        Some(v) => v,
        None => return response::json_err(400, "invalid id"),
    };
    match store.get_rule(id).await {
        Ok(Some(rule)) => response::json_ok(serde_json::json!({"rule": rule})),
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

pub(crate) async fn rules_create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
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

    // Validate and parse condition JSON
    let parsed_condition = match serde_json::from_str::<serde_json::Value>(&body.rule_json) {
        Ok(v) => v,
        Err(e) => return response::json_err(400, &format!("invalid condition JSON: {e}")),
    };

    // Reconstruct full Rule JSON for the scoring pipeline (active_rule_jsons -> rules::score
    // expects {name, audience_tag, condition, score_delta}).
    let full_rule = serde_json::json!({
        "name": body.name,
        "audience_tag": body.audience_tag.clone().unwrap_or_else(|| "default".into()),
        "condition": parsed_condition,
        "score_delta": body.score_delta.unwrap_or(0.0),
    });
    let full_rule_str = serde_json::to_string(&full_rule).unwrap_or(body.rule_json.clone());

    match store
        .insert_rule(
            &body.name,
            &full_rule_str,
            &body.audience_tag.unwrap_or_else(|| "default".into()),
            body.signal_type.as_deref(),
            body.score_delta.unwrap_or(0.0),
        )
        .await
    {
        Ok(Some(id)) => match store.get_rule(id).await {
            Ok(Some(rule)) => response::json_ok(serde_json::json!({"rule": rule})),
            _ => response::json_ok(serde_json::json!({"id": id})),
        },
        Ok(None) => response::json_err(500, "rule creation returned no id"),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

#[derive(Deserialize)]
struct UpdateRuleBody {
    name: Option<String>,
    rule_json: Option<String>,
    enabled: Option<bool>,
    signal_type: Option<Option<String>>,
}

pub(crate) async fn rules_update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match params::param_i64(&ctx, "id") {
        Some(v) => v,
        None => return response::json_err(400, "invalid id"),
    };
    let body: UpdateRuleBody = match req.json().await {
        Ok(b) => b,
        Err(_) => return response::json_err(400, "invalid JSON body"),
    };

    // If rule_json is being updated, wrap condition-only JSON in full Rule JSON
    let mut rule_json_for_store: Option<String> = None;
    if let Some(ref cond_json) = body.rule_json {
        if let Ok(Some(existing)) = store.get_rule(id).await {
            let full_rule = serde_json::json!({
                "name": body.name.as_deref().unwrap_or(&existing.name),
                "audience_tag": existing.audience_tag,
                "condition": serde_json::from_str::<serde_json::Value>(cond_json).unwrap_or_default(),
                "score_delta": existing.score_delta,
            });
            rule_json_for_store = Some(serde_json::to_string(&full_rule).unwrap_or_else(|_| cond_json.clone()));
        } else {
            return response::json_err(404, "rule not found for update");
        }
    }

    if let Err(e) = store
        .update_rule(
            id,
            body.name.as_deref(),
            rule_json_for_store.as_deref().or(body.rule_json.as_deref()),
            body.enabled,
            body.signal_type.as_ref().map(|opt| opt.as_deref()),
        )
        .await
    {
        return response::json_err_internal(&e.to_string());
    }
    match store.get_rule(id).await {
        Ok(Some(rule)) => response::json_ok(serde_json::json!({"rule": rule})),
        Ok(None) => response::json_err(404, "rule not found"),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn rules_delete(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match params::param_i64(&ctx, "id") {
        Some(v) => v,
        None => return response::json_err(400, "invalid id"),
    };
    match store.delete_rule(id).await {
        Ok(()) => response::json_ok(serde_json::json!({"status": "disabled", "id": id})),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}
