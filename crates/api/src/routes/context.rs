use context_engine::builder::ContextBuilder;
use context_engine::types::{ContextRequest, ContextResponse};
use serde_json::json;
use store::D1Store;
use worker::*;

use crate::shared::response;

/// POST /api/internal/context
///
/// Build a cognitive context snapshot from a user query.
/// Internal endpoint — not exposed to external clients.
pub async fn internal_context(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let body: ContextRequest = match req.json().await {
        Ok(b) => b,
        Err(_) => return response::json_err(400, "invalid request body"),
    };

    let store = match ctx.env.d1("DB") {
        Ok(db) => D1Store::new(db),
        Err(e) => {
            console_log!("[context] D1 binding failed: {e}");
            return response::json_err(503, "service unavailable");
        }
    };

    let builder = ContextBuilder::new(store);

    match builder.build(&body.query, body.options, None, None).await {
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
