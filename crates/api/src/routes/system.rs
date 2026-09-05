//! System/aggregation route handlers.
//! Health, pipeline status, dashboard, stats, categories, tags, signals, and debug endpoints.
//!
//! Each handler delegates its D1 reads to [`application::SystemService`]; only
//! the KV cache-aside (categories/tags) and KV pipeline-metrics enrichment
//! (pipeline_status) stay route-level, since KV is delivery-layer infra.

use application::{SystemService, TrustService};
use serde_json::json;
use worker::*;

use store::Store;

use crate::shared::{params, response};

pub(crate) async fn cors_preflight(_req: Request, _ctx: RouteContext<Store>) -> Result<Response> {
    response::json_ok(serde_json::json!({}))
}

pub(crate) async fn ping(_req: Request, _ctx: RouteContext<Store>) -> Result<Response> {
    Response::ok("pong")
}

pub(crate) async fn debug_feeds_due(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = SystemService::new(ctx.data.clone());
    let now = (js_sys::Date::now() / 1000.0) as i64;
    match service.feeds_due(now).await {
        Ok(feeds) => response::json_ok(
            json!({"now": now, "feeds_due": feeds.len(), "feeds": feeds.iter().map(|f| json!({"id": f.id, "title": f.title, "last_fetched_at": f.last_fetched_at, "fetch_interval_sec": f.fetch_interval_sec, "extraction_level": f.extraction_level})).collect::<Vec<_>>()}),
        ),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn pipeline_status(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = SystemService::new(ctx.data.clone());
    let now = (js_sys::Date::now() / 1000.0) as i64;
    match service.pipeline_status(now).await {
        Ok(mut status) => {
            // Enrich with pipeline timing metrics from KV (route-level infra)
            if let Ok(cache) = ctx.env.kv("CACHE") {
                if let Ok(Some(metrics_str)) = cache.get("pipeline_metrics").text().await {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&metrics_str) {
                        status["metrics"] = v;
                    }
                }
            }
            response::json_ok(status)
        }
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn health(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = SystemService::new(ctx.data.clone());
    match service.health_stats().await {
        Ok(s) => response::json_ok(json!({"status": "ok", "stats": s})),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn dashboard(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = SystemService::new(ctx.data.clone());
    match service.dashboard().await {
        Ok((stats, feeds)) => response::json_ok(json!({"status": "ok", "stats": stats, "feeds": feeds})),
        Err(_) => response::json_err(500, "dashboard query failed"),
    }
}

pub(crate) async fn stats(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = SystemService::new(ctx.data.clone());
    match service.score_stats().await {
        Ok((scores, trend)) => response::json_ok(json!({"score_distribution": scores, "articles_per_day": trend})),
        Err(_) => response::json_err(500, "stats query failed"),
    }
}

pub(crate) async fn categories(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let cache_key = "v1:categories";
    if let Some(cached) = crate::cache_get(&ctx.env, cache_key).await {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cached) {
            let mut resp = Response::from_json(&v)?;
            response::cors_headers(&mut resp);
            return Ok(resp);
        }
    }
    let service = SystemService::new(ctx.data.clone());
    match service.categories().await {
        Ok(list) => {
            let result = serde_json::json!({"categories": list.into_iter().map(|(cat, count)| serde_json::json!({"category": cat, "article_count": count})).collect::<Vec<_>>()});
            if let Ok(json_str) = serde_json::to_string(&result) {
                crate::cache_put(&ctx.env, cache_key, &json_str, 120).await;
            }
            response::json_ok(result)
        }
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn tags(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let cache_key = "v1:tags";
    if let Some(cached) = crate::cache_get(&ctx.env, cache_key).await {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cached) {
            let mut resp = Response::from_json(&v)?;
            response::cors_headers(&mut resp);
            return Ok(resp);
        }
    }
    let service = SystemService::new(ctx.data.clone());
    match service.tags().await {
        Ok(list) => {
            let result = serde_json::json!({"tags": list.into_iter().map(|(tag, count)| serde_json::json!({"tag": tag, "count": count})).collect::<Vec<_>>()});
            if let Ok(json_str) = serde_json::to_string(&result) {
                crate::cache_put(&ctx.env, cache_key, &json_str, 120).await;
            }
            response::json_ok(result)
        }
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn trust(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = TrustService::new(ctx.data.clone());
    match service.build().await {
        Ok(report) => response::json_ok(report),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn intelligence_signals(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = SystemService::new(ctx.data.clone());
    let now = (js_sys::Date::now() / 1000.0) as i64;
    match service.signals_today(now).await {
        Ok(signals) => response::json_ok(json!({
            "date": params::fmt_date_ymd(now),
            "generated_at": params::fmt_datetime_iso(now),
            "signals": signals,
        })),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}
