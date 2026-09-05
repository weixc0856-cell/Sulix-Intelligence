use context_engine::builder::ContextBuilder;
use context_engine::types::{ContextRequest, ContextResponse};
use infrastructure::context_repository::D1ContextRepository;
use serde_json::json;
use store::Store;
use worker::*;

use super::response;

/// POST /api/internal/context
///
/// Build a cognitive context snapshot from a user query.
/// Internal endpoint — not exposed to external clients.
pub(crate) async fn internal_context(mut req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let body: ContextRequest = match req.json().await {
        Ok(b) => b,
        Err(_) => return response::json_err(400, "invalid request body"),
    };

    let store = ctx.data.clone();

    // Composition-root wiring: domain engine + adapter are assembled here.
    let builder = ContextBuilder::new(D1ContextRepository::new(store));

    match builder.build(&body.query, body.options, None).await {
        Ok(context) => {
            let resp = ContextResponse { snapshot_id: context.snapshot_id.clone(), context };
            response::json_ok(json!(resp))
        }
        Err(e) => {
            console_log!("[context] build failed: {e}");
            response::json_err(500, "context building failed")
        }
    }
}
