//! Signal Intelligence route handlers.
//!
//! - `GET /api/intelligence/radar` — Intelligence Radar dashboard

use serde_json::json;
use worker::*;

use store::Store;

use crate::shared::response;

/// GET /api/intelligence/radar — Intelligence Radar dashboard.
///
/// Uses the unified SignalQueryService read model to return active
/// signal threads with health scores and related entities.
pub async fn radar(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    use signal_engine::query::SignalQueryService;

    let store = Store::new(ctx.env.d1("DB")?);
    let now = (js_sys::Date::now() / 1000.0) as i64;
    let qs = SignalQueryService::new(&store);

    match qs.radar(now).await {
        Ok(projection) => response::json_ok(json!(projection)),
        Err(e) => {
            console_log!("[Sulix:radar] query failed: {e}");
            response::json_err_internal("radar query failed")
        }
    }
}

/// GET /api/intelligence/signals/:id — Signal Detail page.
///
/// Returns the full SignalDetail DTO for human investigation:
/// thread metadata, health, timeline, evidence, entities, related signals.
pub async fn signal_detail(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = crate::Store::new(ctx.env.d1("DB")?);
    let id = match ctx.param("id").and_then(|s| s.parse::<i64>().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid signal id"),
    };

    match store.load_signal_detail(id).await {
        Ok(Some(detail)) => response::json_ok(serde_json::json!({ "success": true, "signal": detail })),
        Ok(None) => response::json_err(404, "signal not found"),
        Err(e) => {
            console_log!("[Sulix:signal] load_signal_detail failed: {e}");
            response::json_err_internal("signal detail query failed")
        }
    }
}

/// GET /api/intelligence/threads/:id — Signal Thread Detail (Read Model).
///
/// Uses the unified SignalQueryService to build the response:
/// merges instance timeline with signal_events, adds rule-based summary.
pub async fn thread_detail(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    use signal_engine::query::SignalQueryService;

    let store = crate::Store::new(ctx.env.d1("DB")?);
    let id = match ctx.param("id").and_then(|s| s.parse::<i64>().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid thread id"),
    };

    let qs = SignalQueryService::new(&store);
    match qs.thread_detail(id).await {
        Ok(Some(detail)) => response::json_ok(serde_json::json!({ "success": true, "signal": detail })),
        Ok(None) => response::json_err(404, "thread not found"),
        Err(e) => {
            console_log!("[Sulix:thread] thread_detail failed: {e}");
            response::json_err_internal("thread detail query failed")
        }
    }
}
