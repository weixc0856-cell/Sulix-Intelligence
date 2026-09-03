//! Signal Intelligence route handlers.
//!
//! - `GET /api/intelligence/radar` — Intelligence Radar dashboard
//! - `GET /api/intelligence/signals/:id` — Signal Detail
//! - `GET /api/intelligence/signals/:id/provenance` — Signal provenance

use std::collections::BTreeSet;

use serde_json::json;
use worker::*;

use store::Store;

use crate::shared::response;

/// GET /api/intelligence/radar — Intelligence Radar dashboard.
///
/// Uses the RadarProjectionService with batch queries (3 total D1 calls
/// instead of the previous 1+3N pattern) to return active signal threads
/// with health scores, evidence counts, and related entities.
pub async fn radar(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let projection = application::RadarProjectionService::new(store);

    match projection.build(20).await {
        Ok(result) => response::json_ok(json!(result)),
        Err(e) => {
            console_log!("[Sulix:radar] projection failed: {e}");
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

/// GET /api/intelligence/signals/:id/provenance
///
/// Returns provenance summary for a signal: evidence sources, observation count,
/// and confidence. Uses batch resolution: evidence article IDs → feed IDs → sources.
pub async fn signal_provenance(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = crate::Store::new(ctx.env.d1("DB")?);
    let id = match ctx.param("id").and_then(|s| s.parse::<i64>().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid signal id"),
    };

    let detail = match store.load_signal_detail(id).await {
        Ok(Some(d)) => d,
        Ok(None) => return response::json_err(404, "signal not found"),
        Err(e) => {
            console_log!("[Sulix:signal] load_signal_detail failed: {e}");
            return response::json_err_internal("signal detail query failed");
        }
    };

    // Resolve evidence article IDs to feed_ids, then to sources
    let evidence_article_ids: Vec<i64> = detail.evidence_top.iter().map(|a| a.id).collect();
    let mut source_summaries: Vec<store::SourceSummary> = Vec::new();
    let mut seen_feed_ids = BTreeSet::new();

    if !evidence_article_ids.is_empty() {
        if let Ok(articles) = store.articles_by_ids(&evidence_article_ids).await {
            for article in &articles {
                if seen_feed_ids.insert(article.feed_id) {
                    if let Ok(Some(source)) = store.find_source_by_feed(article.feed_id).await {
                        source_summaries.push(source.into());
                    }
                }
            }
        }
    }

    let provenance = store::ProvenanceSummary {
        entity_type: "signal".into(),
        entity_id: format!("SIG-{:06}", id),
        sources: source_summaries,
        observation_count: detail.evidence_top.len(),
        evidence_count: detail.evidence_top.len(),
        confidence: Some(detail.health.score),
    };

    response::json_ok(json!(provenance))
}
