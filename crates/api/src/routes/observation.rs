//! Observation API — read-only access to structured observation records
//! and their lineage chain.

use serde_json::json;
use worker::*;

use store::Store;

use crate::shared::response;

/// GET /api/observations
/// List observations, optionally filtered by ?source_type= and ?source_id=.
pub async fn list(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);

    let url = req.url()?;
    let pairs = url.query_pairs().collect::<Vec<_>>();
    let source_type = pairs.iter().find(|(k, _)| k == "source_type").map(|(_, v)| v.as_ref());
    let source_id = pairs.iter().find(|(k, _)| k == "source_id").map(|(_, v)| v.as_ref());
    let limit = pairs.iter().find(|(k, _)| k == "limit").and_then(|(_, v)| v.parse::<u32>().ok()).unwrap_or(50);
    let offset = pairs.iter().find(|(k, _)| k == "offset").and_then(|(_, v)| v.parse::<u32>().ok()).unwrap_or(0);

    match store.list_observations(source_type, source_id, limit, offset).await {
        Ok(observations) => response::json_ok(json!({ "observations": observations })),
        Err(e) => {
            console_log!("[Sulix:observations] list failed: {e}");
            response::json_err_internal("list observations failed")
        }
    }
}

/// GET /api/observations/:id
pub async fn get(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid observation id"),
    };

    match store.get_observation(id).await {
        Ok(Some(o)) => response::json_ok(json!({ "observation": o })),
        Ok(None) => response::json_err(404, "observation not found"),
        Err(e) => {
            console_log!("[Sulix:observations] get failed: {e}");
            response::json_err_internal("get observation failed")
        }
    }
}

/// GET /api/observations/:id/lineage
/// Full provenance chain: Source → Observation → Signals → Claims → Decisions.
pub async fn lineage(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid observation id"),
    };

    let observation = match store.get_observation(id).await {
        Ok(Some(o)) => o,
        Ok(None) => return response::json_err(404, "observation not found"),
        Err(e) => {
            console_log!("[Sulix:observations] lineage get observation failed: {e}");
            return response::json_err_internal("lineage query failed");
        }
    };

    // Resolve source metadata
    let source =
        if let Some(sid) = observation.registry_source_id { store.find_source(sid).await.ok().flatten() } else { None };

    // Build lineage response
    response::json_ok(json!({
        "observation": observation,
        "source": source,
    }))
}
