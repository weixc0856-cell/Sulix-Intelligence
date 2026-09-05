//! Confidence History API — append-only 置信度演化追踪。

use application::ConfidenceService;
use serde::Serialize;
use serde_json::json;
use worker::*;

use store::{ConfidenceEvent, Store};

use crate::shared::response;

/// Response DTO — 保证未来可以增加 current / trend 而不破坏 API。
#[derive(Serialize)]
pub struct ConfidenceHistoryResponse {
    pub entity_type: String,
    pub entity_id: String,
    pub history: Vec<ConfidenceEvent>,
}

/// GET /api/confidence/:entity_type/:entity_id
///
/// 返回某实体的置信度历史变化轨迹。
/// entity_type: "decision" | "signal" | "claim"
pub async fn history(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = ConfidenceService::new(ctx.data.clone());

    let entity_type = match ctx.param("entity_type") {
        Some(v) => v.to_string(),
        None => return response::json_err(400, "missing entity_type"),
    };
    let entity_id = match ctx.param("entity_id") {
        Some(v) => v.to_string(),
        None => return response::json_err(400, "missing entity_id"),
    };

    match service.history(&entity_type, &entity_id).await {
        Ok(history) => response::json_ok(json!(ConfidenceHistoryResponse { entity_type, entity_id, history })),
        Err(e) => {
            console_log!("[Sulix:confidence] list failed: {e}");
            response::json_err_internal("confidence history query failed")
        }
    }
}
