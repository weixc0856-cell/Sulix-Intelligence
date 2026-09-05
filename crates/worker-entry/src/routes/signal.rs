//! Signal read-model routes owned by the composition root.
//!
//! Migrated from `api` in P3 Round 2 (thread/entities read-model routes) and
//! Phase 2 Checkpoint G (radar / signal detail / provenance). These endpoints
//! assemble store-backed adapters (`D1SignalQuery`) or drive raw-store reads
//! (`load_signal_detail`, `articles_by_ids`, `find_source_by_feed`) plus the
//! `RadarProjectionService`, so they live here in worker-entry where the
//! composition-root store is reachable via `ctx.data.store`.
//! Wiring only — no business logic is copied into this crate.

use std::collections::BTreeSet;

use application::RadarProjectionService;
use composition::ProductionAppServices;
use infrastructure::signal_repository::D1SignalQuery;
use serde_json::json;
use signal_engine::query::SignalQueryService;
use worker::*;

use super::response;

/// GET /api/intelligence/threads/:id — Signal Thread Detail (Read Model).
///
/// Uses the unified SignalQueryService to build the response: merges instance
/// timeline with stored signal_events, adds the rule-based summary. The event
/// log is intentionally left unset (as the migrated api handler did), so the
/// D1 `signal_events` fallback is the timeline source — no behaviour change.
pub(crate) async fn thread_detail(_req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let id = match ctx.param("id").and_then(|s| s.parse::<i64>().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid thread id"),
    };

    let store = ctx.data.store.clone();
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
pub(crate) async fn entities_signals(_req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let id = match ctx.param("id").and_then(|s| s.parse::<i64>().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid entity id"),
    };

    let store = ctx.data.store.clone();
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
pub(crate) async fn entities_threads(req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    entities_signals(req, ctx).await
}

/// GET /api/intelligence/radar — Intelligence Radar dashboard.
///
/// Uses the RadarProjectionService with batch queries (3 total D1 calls
/// instead of the previous 1+3N pattern) to return active signal threads
/// with health scores, evidence counts, and related entities.
pub(crate) async fn radar(_req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let projection = RadarProjectionService::new(ctx.data.store.clone());

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
pub(crate) async fn signal_detail(_req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let store = ctx.data.store.clone();
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
pub(crate) async fn signal_provenance(_req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let store = ctx.data.store.clone();
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
