//! Entity Graph API endpoints.
//!
//! Routes:
//! - `GET /api/intelligence/entities`          — list all entities (paginated)
//! - `GET /api/intelligence/entities/:id`       — entity detail
//! - `GET /api/intelligence/entities/:id/relations` — related entities

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
