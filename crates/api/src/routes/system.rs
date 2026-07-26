//! System/aggregation route handlers.
//! Health, pipeline status, dashboard, stats, categories, tags, signals, and debug endpoints.

use serde_json::json;
use worker::*;

use store::Store;

use crate::shared::{params, response};

pub(crate) async fn cors_preflight(_req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    response::json_ok(serde_json::json!({}))
}

pub(crate) async fn ping(_req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    Response::ok("pong")
}

pub(crate) async fn debug_feeds_due(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let now = (js_sys::Date::now() / 1000.0) as i64;
    match store.feeds_due_for_fetch(now, None).await {
        Ok(feeds) => response::json_ok(
            json!({"now": now, "feeds_due": feeds.len(), "feeds": feeds.iter().map(|f| json!({"id": f.id, "title": f.title, "last_fetched_at": f.last_fetched_at, "fetch_interval_sec": f.fetch_interval_sec, "extraction_level": f.extraction_level})).collect::<Vec<_>>()}),
        ),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn pipeline_status(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let now = (js_sys::Date::now() / 1000.0) as i64;
    match store.pipeline_status(now).await {
        Ok(mut status) => {
            // Enrich with pipeline timing metrics from KV
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

pub(crate) async fn health(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    match store.health_stats().await {
        Ok(s) => response::json_ok(json!({"status": "ok", "stats": s})),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn dashboard(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    match (store.health_stats().await, store.feed_stats().await) {
        (Ok(stats), Ok(feeds)) => response::json_ok(json!({"status": "ok", "stats": stats, "feeds": feeds})),
        _ => response::json_err(500, "dashboard query failed"),
    }
}

pub(crate) async fn stats(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    match (store.score_distribution().await, store.article_trend(14).await) {
        (Ok(scores), Ok(trend)) => response::json_ok(json!({"score_distribution": scores, "articles_per_day": trend})),
        _ => response::json_err(500, "stats query failed"),
    }
}

pub(crate) async fn categories(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cache_key = "v1:categories";
    if let Some(cached) = crate::cache_get(&ctx.env, cache_key).await {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cached) {
            let mut resp = Response::from_json(&v)?;
            response::cors_headers(&mut resp);
            return Ok(resp);
        }
    }
    let store = Store::new(ctx.env.d1("DB")?);
    match store.categories_summary().await {
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

pub(crate) async fn tags(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cache_key = "v1:tags";
    if let Some(cached) = crate::cache_get(&ctx.env, cache_key).await {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cached) {
            let mut resp = Response::from_json(&v)?;
            response::cors_headers(&mut resp);
            return Ok(resp);
        }
    }
    let store = Store::new(ctx.env.d1("DB")?);
    match store.tags_summary().await {
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

pub(crate) async fn trust(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);

    // Health stats (total articles, feeds)
    let health = store.health_stats().await.ok();

    // Decision stats (evaluations, accuracy)
    let decision_stats = store.decision_stats().await.ok();

    // Source reliability (all sources with trust scores)
    let sources = store.list_sources(None, None, 100, 0).await.unwrap_or_default();

    // Source reliability ranking: filter to sources with trust scores
    let source_reliability: Vec<serde_json::Value> = sources
        .iter()
        .filter(|s| s.trust_score.is_some())
        .map(|s| {
            json!({
                "name": s.name.as_deref().unwrap_or("Unknown"),
                "tier": s.tier,
                "trust_score": s.trust_score.unwrap_or(0.0),
                "verified": s.verified,
                "policy": s.policy,
            })
        })
        .collect();

    // Aggregate total counts
    let total_signals = 0u64; // signal_engine handles this separately
    let total_sources = sources.len();

    // Compute accuracy rate from decision stats
    let accuracy_rate = decision_stats.as_ref().and_then(|ds| {
        let total = ds.evaluation_summary.total_evaluated;
        if total > 0 {
            Some(ds.evaluation_summary.confirmed as f64 / total as f64)
        } else {
            None
        }
    });

    response::json_ok(json!({
        "signals_analyzed": health.as_ref().map(|h| h.article_count).unwrap_or(0),
        "active_sources": total_sources,
        "total_decisions": decision_stats.as_ref().map(|ds| ds.total_decisions).unwrap_or(0),
        "total_evaluations": decision_stats.as_ref().map(|ds| ds.evaluation_summary.total_evaluated).unwrap_or(0),
        "accuracy_rate": accuracy_rate,
        "source_reliability": source_reliability,
        "evaluation_summary": decision_stats.map(|ds| ds.evaluation_summary),
    }))
}

pub(crate) async fn intelligence_signals(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let now = (js_sys::Date::now() / 1000.0) as i64;
    match store.signals_today(now).await {
        Ok(signals) => response::json_ok(json!({
            "date": params::fmt_date_ymd(now),
            "generated_at": params::fmt_datetime_iso(now),
            "signals": signals,
        })),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}
