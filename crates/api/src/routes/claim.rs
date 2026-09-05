//! Claim API — 查询专用，无公开写 API。
//! Claim 由 Pipeline Agent 内部生成。

use application::ClaimService;
use serde_json::json;
use worker::*;

use store::Store;

use crate::shared::response;

/// GET /api/claims/:id — Claim detail with evidence.
pub async fn detail_with_evidence(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = ClaimService::new(ctx.data.clone());
    let id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid claim id"),
    };

    match service.detail(id).await {
        Ok(Some((claim, evidence))) => response::json_ok(json!({
            "success": true,
            "claim": {
                "id": format!("CLM-{:06}", claim.id),
                "statement": claim.statement,
                "claim_type": claim.claim_type,
                "status": claim.status,
                "created_at": claim.created_at,
                "evidence": evidence.iter().map(|e| json!({
                    "article_id": e.article_id,
                    "strength": e.strength,
                    "relation": e.relation,
                })).collect::<Vec<_>>(),
            }
        })),
        Ok(None) => response::json_err(404, "claim not found"),
        Err(e) => {
            console_log!("[Sulix:claims] get failed: {e}");
            response::json_err_internal("get claim failed")
        }
    }
}
