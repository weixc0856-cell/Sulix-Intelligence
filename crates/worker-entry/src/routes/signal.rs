//! Signal read-model routes owned by the composition root.
//!
//! Migrated from `api` in P3 Round 2: signal-engine's query read model is now
//! store-decoupled (`SignalQuery` port), so the HTTP adapters that consume it
//! live here in worker-entry where the infrastructure adapters are assembled.
//! Wiring only — builds `D1SignalQuery`, delegates to the `SignalQueryService`
//! read model, maps to HTTP. No business logic is copied into this crate.

use infrastructure::signal_repository::D1SignalQuery;
use serde_json::json;
use signal_engine::query::SignalQueryService;
use store::Store;
use worker::*;

use super::response;

/// GET /api/intelligence/threads/:id — Signal Thread Detail (Read Model).
///
/// Uses the unified SignalQueryService to build the response: merges instance
/// timeline with stored signal_events, adds the rule-based summary. The event
/// log is intentionally left unset (as the migrated api handler did), so the
/// D1 `signal_events` fallback is the timeline source — no behaviour change.
pub(crate) async fn thread_detail(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let id = match ctx.param("id").and_then(|s| s.parse::<i64>().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid thread id"),
    };

    let store = ctx.data.clone();
    let query = D1SignalQuery::new(&store);
    let qs = SignalQueryService::new(&query);

    match qs.thread_detail(id).await {
        Ok(Some(detail)) => response::json_ok(json!({ "success": true, "signal": detail })),
        Ok(None) => response::json_err(404, "thread not found"),
        Err(e) => {
            console_log!("[Sulix:thread] thread_detail failed: {e}");
            response::json_err_internal("thread detail query failed")
        }
    }
}

/// GET /api/intelligence/entities/:id/signals — signal threads for an entity.
///
/// Returns thread-level summaries from the SignalQueryService read model.
pub(crate) async fn entities_signals(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let id = match ctx.param("id").and_then(|s| s.parse::<i64>().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid entity id"),
    };

    let store = ctx.data.clone();
    let query = D1SignalQuery::new(&store);
    let qs = SignalQueryService::new(&query);

    match qs.entity_threads(id, 20).await {
        Ok(threads) => response::json_ok(json!({ "success": true, "signals": threads })),
        Err(e) => {
            console_log!("[Sulix:entities] entity threads failed: {e}");
            response::json_err_internal("entity signal query failed")
        }
    }
}

/// GET /api/intelligence/entities/:id/threads — semantic alias for `signals`.
pub(crate) async fn entities_threads(req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    entities_signals(req, ctx).await
}
