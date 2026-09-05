//! Decision Graph projection HTTP handler.
//!
//! - `GET /api/projections/decision-graph` — Decision-centric graph projection

use serde_json::json;
use worker::*;

use store::Store;

use crate::shared::{params, response};

/// GET /api/projections/decision-graph — Decision Intelligence Graph.
///
/// Returns a render-ready node+edge projection of recent decisions,
/// their associated signals, and outcomes.
/// Query params: `?limit=20`
pub async fn decision_graph(req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let store = ctx.data.clone();
    let service = application::GraphProjectionService::new(store);
    let url = req.url()?;
    let limit = params::parse_limit(&url);

    match service.build_decision_graph(limit).await {
        Ok(graph) => response::json_ok(json!(graph)),
        Err(e) => {
            console_log!("[Sulix:graph] decision_graph failed: {e}");
            response::json_err_internal("decision graph query failed")
        }
    }
}

/// POST /api/projections/decision-graph/{id}/expand
///
/// Expand a node to reveal its neighbors. Returns additional nodes and edges.
pub async fn expand(mut req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    use application::ExpandRequest;

    let store = ctx.data.clone();
    let service = application::GraphProjectionService::new(store);

    let node_id = match ctx.param("id") {
        Some(id) => id.to_string(),
        None => return response::json_err(400, "missing node id"),
    };

    let expand_req: ExpandRequest = match req.json().await {
        Ok(r) => r,
        Err(_) => ExpandRequest { depth: Some(1), include: None },
    };

    match service.expand(&node_id, expand_req).await {
        Ok(result) => response::json_ok(json!(result)),
        Err(e) => {
            console_log!("[Sulix:graph] expand failed: {e}");
            response::json_err_internal("graph expand failed")
        }
    }
}
