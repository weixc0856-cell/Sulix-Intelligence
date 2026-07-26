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
pub async fn decision_graph(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
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
