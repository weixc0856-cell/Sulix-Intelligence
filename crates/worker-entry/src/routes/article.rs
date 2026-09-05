//! Article raw-content endpoint — composition-root owned.
//!
//! `GET /api/articles/:id/content`
//!
//! Migrated from `api` in Phase 2 (Checkpoint G): this endpoint resolves the
//! source policy through `content_governance` and streams the stored HTML from
//! R2, so it is an infrastructure-facing endpoint that lives in worker-entry
//! next to the other adapters. Wiring only — no business logic is copied here.

use application::ProductionAppServices;
use serde_json::json;
use worker::*;

use super::response::{json_err, json_err_internal, json_ok};

fn param_i64<D>(ctx: &RouteContext<D>, name: &str) -> Option<i64> {
    ctx.param(name).and_then(|s| s.parse().ok())
}

/// GET /api/articles/:id/content
pub(crate) async fn article_content(_req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let store = ctx.data.store.clone();
    let id = match param_i64(&ctx, "id") {
        Some(v) => v,
        None => return json_err(400, "missing id"),
    };

    // Resolve source and check policy before serving content
    if let Ok(Some(article)) = store.article_by_id(id).await {
        if let Ok(Some(source)) = store.find_source_by_feed(article.feed_id).await {
            let decision = content_governance::evaluate_policy(&source);
            if decision.serving == content_governance::ServingPermission::Denied {
                return json_err(403, "Content access denied by source policy");
            }
        }
    }

    match store.get_raw_content_key(id).await {
        Ok(Some(k)) => {
            let bucket = match ctx.env.bucket("RAW_CONTENT") {
                Ok(b) => b,
                Err(e) => return json_err_internal(&format!("RAW_CONTENT bucket: {e}")),
            };
            match bucket.get(&k).execute().await {
                Ok(Some(obj)) => match obj.body() {
                    Some(body) => match body.text().await {
                        Ok(t) => json_ok(json!({"id": id, "content": t, "format": "html", "source": "r2"})),
                        Err(e) => json_err_internal(&format!("body read: {e}")),
                    },
                    None => json_err(500, "R2 object has no body"),
                },
                Ok(None) => json_err(404, "content not found in storage"),
                Err(e) => json_err_internal(&format!("R2 read: {e}")),
            }
        }
        Ok(None) => json_err(404, "no raw content for this article"),
        Err(e) => json_err_internal(&e.to_string()),
    }
}
