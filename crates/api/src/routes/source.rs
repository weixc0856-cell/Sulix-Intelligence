//! Source Registry API — governance metadata for content sources.
//! Write operations (POST/PUT/DELETE) are admin-only.

use serde_json::json;
use worker::*;

use application::{NewSource, ProductionAppServices};

use crate::shared::response;

/// GET /api/sources
/// List all sources with optional ?tier= and ?policy= filters.
pub async fn sources_list(req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let service = &ctx.data.source;

    let url = req.url()?;
    let pairs = url.query_pairs().collect::<Vec<_>>();
    let tier = pairs.iter().find(|(k, _)| k == "tier").map(|(_, v)| v.as_ref());
    let policy = pairs.iter().find(|(k, _)| k == "policy").map(|(_, v)| v.as_ref());
    let limit = pairs.iter().find(|(k, _)| k == "limit").and_then(|(_, v)| v.parse::<u32>().ok()).unwrap_or(50);
    let offset = pairs.iter().find(|(k, _)| k == "offset").and_then(|(_, v)| v.parse::<u32>().ok()).unwrap_or(0);

    match service.list(tier, policy, limit, offset).await {
        Ok(sources) => response::json_ok(json!({ "sources": sources })),
        Err(e) => {
            console_log!("[Sulix:sources] list failed: {e}");
            response::json_err_internal("list sources failed")
        }
    }
}

/// GET /api/sources/:id
pub async fn sources_get(_req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let service = &ctx.data.source;
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid source id"),
    };

    match service.get(id).await {
        Ok(Some(s)) => response::json_ok(json!({ "source": s })),
        Ok(None) => response::json_err(404, "source not found"),
        Err(e) => {
            console_log!("[Sulix:sources] get failed: {e}");
            response::json_err_internal("get source failed")
        }
    }
}

/// POST /api/sources — Create a new source entry.
pub async fn sources_create(mut req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let service = &ctx.data.source;
    let body: NewSource = match req.json().await {
        Ok(b) => b,
        Err(_) => return response::json_err(400, "invalid request body"),
    };

    match service.create(&body).await {
        Ok(id) => response::json_ok(json!({ "id": id })),
        Err(e) => {
            console_log!("[Sulix:sources] create failed: {e}");
            response::json_err_internal("create source failed")
        }
    }
}

/// PUT /api/sources/:id — Update a source entry.
pub async fn sources_update(mut req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let service = &ctx.data.source;
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid source id"),
    };

    let body: NewSource = match req.json().await {
        Ok(b) => b,
        Err(_) => return response::json_err(400, "invalid request body"),
    };

    // feed_id preservation is an application invariant (SourceService::update).
    match service.update(id, &body).await {
        Ok(new_id) => response::json_ok(json!({ "id": new_id })),
        Err(e) => {
            console_log!("[Sulix:sources] update failed: {e}");
            response::json_err_internal("update source failed")
        }
    }
}

/// DELETE /api/sources/:id
pub async fn sources_delete(_req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let service = &ctx.data.source;
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid source id"),
    };

    match service.delete(id).await {
        Ok(_) => response::json_ok(json!({ "deleted": true })),
        Err(e) => {
            console_log!("[Sulix:sources] delete failed: {e}");
            response::json_err_internal("delete source failed")
        }
    }
}
