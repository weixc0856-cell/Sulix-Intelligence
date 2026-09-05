//! Entity Graph API endpoints.
//!
//! Routes:
//! - `GET /api/intelligence/entities`                     — list all entities (paginated)
//! - `GET /api/intelligence/entities/:id`                  — entity detail
//! - `GET /api/intelligence/entities/:id/activity`         — entity activity summary (7d)
//! - `GET /api/intelligence/entities/:id/articles`         — entity evidence articles (paginated)
//! - `GET /api/intelligence/entities/:id/relations`        — related entities

use crate::{json_err, json_err_internal, json_ok, param_i64, parse_limit, parse_offset};
use application::ProductionAppServices;
use worker::*;

pub async fn entities_list(req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let service = &ctx.data.entity;
    let url = req.url()?;
    let limit = parse_limit(&url);
    let offset = parse_offset(&url);

    match service.list(limit, offset).await {
        Ok(entities) => json_ok(serde_json::json!({
            "success": true,
            "entities": entities,
            "limit": limit,
            "offset": offset,
        })),
        Err(e) => json_err_internal(&e.to_string()),
    }
}

pub async fn entities_get(_req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let service = &ctx.data.entity;
    let id = match param_i64(&ctx, "id") {
        Some(v) => v,
        None => return json_err(400, "invalid entity id"),
    };

    match service.get(id).await {
        Ok(Some(entity)) => json_ok(serde_json::json!({ "success": true, "entity": entity })),
        Ok(None) => json_err(404, "entity not found"),
        Err(e) => json_err_internal(&e.to_string()),
    }
}

pub async fn entities_get_relations(_req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let service = &ctx.data.entity;
    let id = match param_i64(&ctx, "id") {
        Some(v) => v,
        None => return json_err(400, "invalid entity id"),
    };

    match service.relations(id, 50).await {
        Ok(relations) => json_ok(serde_json::json!({ "success": true, "relations": relations })),
        Err(e) => json_err_internal(&e.to_string()),
    }
}

/// GET /api/intelligence/entities/:id/activity
pub async fn entities_activity(_req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let service = &ctx.data.entity;
    let id = match param_i64(&ctx, "id") {
        Some(v) => v,
        None => return json_err(400, "invalid entity id"),
    };
    // The clock is read here (js_sys); the 7-day window is owned by the service.
    let now = (js_sys::Date::now() / 1000.0) as i64;

    match service.activity(id, now).await {
        Ok(summary) => json_ok(serde_json::json!({ "success": true, "activity": summary, "entity_id": id })),
        Err(e) => json_err_internal(&e.to_string()),
    }
}

/// GET /api/intelligence/entities/:id/articles
pub async fn entities_articles(req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let service = &ctx.data.entity;
    let id = match param_i64(&ctx, "id") {
        Some(v) => v,
        None => return json_err(400, "invalid entity id"),
    };
    let url = req.url()?;
    let limit = parse_limit(&url);
    let offset = parse_offset(&url);

    match service.articles(id, limit, offset).await {
        Ok(articles) => json_ok(serde_json::json!({
            "success": true,
            "articles": articles,
            "entity_id": id,
            "limit": limit,
            "offset": offset,
        })),
        Err(e) => json_err_internal(&e.to_string()),
    }
}
