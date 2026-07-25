//! Entity Graph API endpoints.
//!
//! Routes:
//! - `GET /api/intelligence/entities`                     — list all entities (paginated)
//! - `GET /api/intelligence/entities/:id`                  — entity detail
//! - `GET /api/intelligence/entities/:id/activity`         — entity activity summary (7d)
//! - `GET /api/intelligence/entities/:id/articles`         — entity evidence articles (paginated)
//! - `GET /api/intelligence/entities/:id/relations`        — related entities

use crate::{json_err, json_err_internal, json_ok, param_i64, parse_limit, parse_offset};
use store::Store;
use worker::*;

pub async fn entities_list(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let url = req.url()?;
    let limit = parse_limit(&url);
    let offset = parse_offset(&url);

    match store.list_entities(limit, offset).await {
        Ok(entities) => json_ok(serde_json::json!({
            "success": true,
            "entities": entities,
            "limit": limit,
            "offset": offset,
        })),
        Err(e) => json_err_internal(&e.to_string()),
    }
}

pub async fn entities_get(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") {
        Some(v) => v,
        None => return json_err(400, "invalid entity id"),
    };

    match store.entity_detail(id).await {
        Ok(Some(entity)) => json_ok(serde_json::json!({ "success": true, "entity": entity })),
        Ok(None) => json_err(404, "entity not found"),
        Err(e) => json_err_internal(&e.to_string()),
    }
}

/// GET /api/intelligence/entities/:id/signals
///
/// Returns signal threads anchored to this entity. Uses the unified
/// SignalQueryService read model for accurate thread-level summaries.
pub async fn entities_signals(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    use signal_engine::query::SignalQueryService;

    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") {
        Some(v) => v,
        None => return json_err(400, "invalid entity id"),
    };
    let qs = SignalQueryService::new(&store);
    match qs.entity_threads(id, 20).await {
        Ok(threads) => json_ok(serde_json::json!({ "success": true, "signals": threads })),
        Err(e) => json_err_internal(&e.to_string()),
    }
}

/// GET /api/intelligence/entities/:id/threads
///
/// Semantic alias for `entities_signals` with the new naming convention.
/// Returns signal thread summaries anchored to this entity.
pub async fn entities_threads(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    entities_signals(req, ctx).await
}

pub async fn entities_get_relations(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") {
        Some(v) => v,
        None => return json_err(400, "invalid entity id"),
    };

    match store.entity_relations(id, 50).await {
        Ok(relations) => json_ok(serde_json::json!({ "success": true, "relations": relations })),
        Err(e) => json_err_internal(&e.to_string()),
    }
}

/// GET /api/intelligence/entities/:id/activity
pub async fn entities_activity(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") {
        Some(v) => v,
        None => return json_err(400, "invalid entity id"),
    };
    let now = (js_sys::Date::now() / 1000.0) as i64;

    match store.entity_activity_summary(id, now, 7).await {
        Ok(summary) => json_ok(serde_json::json!({ "success": true, "activity": summary, "entity_id": id })),
        Err(e) => json_err_internal(&e.to_string()),
    }
}

/// GET /api/intelligence/entities/:id/articles
pub async fn entities_articles(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") {
        Some(v) => v,
        None => return json_err(400, "invalid entity id"),
    };
    let url = req.url()?;
    let limit = parse_limit(&url);
    let offset = parse_offset(&url);

    match store.entity_articles(id, limit, offset).await {
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
