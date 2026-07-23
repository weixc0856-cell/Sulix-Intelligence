//! HTTP routes with CORS support for the Sulix Intelligence backend.
//! All responses include `Access-Control-Allow-Origin: *` so the API can
//! be consumed from the Astro frontend (even on a different domain) and
//! from browser-based dev tools without a proxy.

use serde::Deserialize;
use serde_json::{json, Value};
use worker::wasm_bindgen::JsValue;
use worker::*;
// ---- KV cache helpers ---
async fn cache_get(env: &Env, key: &str) -> Option<String> {
    let kv = env.kv("CACHE").ok()?;
    kv.get(key).text().await.ok().flatten()
}

async fn cache_put(env: &Env, key: &str, value: &str, ttl: u64) {
    if let Ok(kv) = env.kv("CACHE") {
        let _ = kv.put(key, value).unwrap().expiration_ttl(ttl).execute().await;
    }
}


use search::D1FtsSearch;
use store::Store;

mod strategies;
mod semantic;
mod rebuild;

fn parse_limit(url: &Url) -> u32 {
    url.query_pairs().find(|(k, _)| k == "limit").and_then(|(_, v)| v.parse().ok()).unwrap_or(30)
}

fn parse_offset(url: &Url) -> u32 {
    url.query_pairs().find(|(k, _)| k == "offset").and_then(|(_, v)| v.parse().ok()).unwrap_or(0)
}

fn cors_headers(resp: &mut Response) {
    let h = resp.headers_mut();
    let _ = h.set("Access-Control-Allow-Origin", "*");
    let _ = h.set("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS");
    let _ = h.set("Access-Control-Allow-Headers", "Content-Type");
    let _ = h.set("X-Content-Type-Options", "nosniff");
    // Aggregations get a longer cache than raw article data
    let _ = h.set("Cache-Control", "public, max-age=60");
}

fn json_ok(v: Value) -> Result<Response> {
    let mut resp = Response::from_json(&v)?;
    cors_headers(&mut resp);
    Ok(resp)
}
fn json_err(status: u16, msg: &str) -> Result<Response> {
    let mut resp = Response::error(msg, status)?;
    cors_headers(&mut resp);
    Ok(resp)
}
fn param_i64(ctx: &RouteContext<()>, name: &str) -> Option<i64> { ctx.param(name)?.parse().ok() }

/// Format a unix timestamp (seconds) as YYYY-MM-DD using js_sys::Date.
fn fmt_date_ymd(ts_secs: i64) -> String {
    let d = js_sys::Date::new(&JsValue::from_f64((ts_secs as f64) * 1000.0));
    format!("{:04}-{:02}-{:02}", d.get_full_year(), d.get_month() + 1, d.get_date())
}

/// Format a unix timestamp (seconds) as ISO 8601 UTC.
fn fmt_datetime_iso(ts_secs: i64) -> String {
    let d = js_sys::Date::new(&JsValue::from_f64((ts_secs as f64) * 1000.0));
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        d.get_full_year(),
        d.get_month() + 1,
        d.get_date(),
        d.get_hours(),
        d.get_minutes(),
        d.get_seconds(),
    )
}

pub fn router() -> Router<'static, ()> {
    Router::new()
        // CORS preflight
        .options_async("/api/*path", |_req, _ctx| async move { json_ok(json!({})) })
        // Health / debug
.get_async("/api/ping", |_req, _ctx| async move { Response::ok("pong") })
        .get_async("/api/pipeline/status", pipeline_status)
        .get_async("/api/health", health)
        .get_async("/api/debug/feeds-due", debug_feeds_due)
        // Signal Strategies preview
        .post_async("/api/strategies/preview", strategies::preview)
        .post_async("/api/articles/search", semantic::semantic_search)
        .post_async("/api/admin/rebuild-embeddings", rebuild::rebuild_embeddings)
        // Aggregations
        .get_async("/api/dashboard", dashboard)
        .get_async("/api/stats", stats)
        .get_async("/api/categories", categories)
.get_async("/api/tags", tags)
        .get_async("/api/intelligence/signals", intelligence_signals)
        // Feed CRUD
        .get_async("/api/feeds", feeds_list)
        .post_async("/api/feeds", feeds_create)
        .get_async("/api/feeds/:id", feeds_get)
        .put_async("/api/feeds/:id", feeds_update)
        .delete_async("/api/feeds/:id", feeds_delete)
        // Article endpoints
        .get_async("/api/articles/latest", latest_articles)
        .get_async("/api/articles/trending", trending)
        .get_async("/api/articles/batch", articles_batch)
        .get_async("/api/articles/search", search_articles)
        .get_async("/api/articles/:id/related", article_related)
        .get_async("/api/articles/:id/adjacent", article_adjacent)
        .get_async("/api/articles/:id", article_detail)
        .get_async("/api/articles/:id/content", article_content)
        // Rules CRUD
        .get_async("/api/rules", rules_list)
        .post_async("/api/rules", rules_create)
        .get_async("/api/rules/:id", rules_get)
        .put_async("/api/rules/:id", rules_update)
        .delete_async("/api/rules/:id", rules_delete)
}

// ---- Handlers ----

async fn debug_feeds_due(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let now = (js_sys::Date::now() / 1000.0) as i64;
    match store.feeds_due_for_fetch(now, None).await {
        Ok(feeds) => json_ok(json!({"now": now, "feeds_due": feeds.len(), "feeds": feeds.iter().map(|f| json!({"id": f.id, "title": f.title, "last_fetched_at": f.last_fetched_at, "fetch_interval_sec": f.fetch_interval_sec, "extraction_level": f.extraction_level})).collect::<Vec<_>>()})),
        Err(e) => json_err(500, &e.to_string()),
    }
}

async fn pipeline_status(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let now = (js_sys::Date::now() / 1000.0) as i64;
    match store.pipeline_status(now).await {
        Ok(status) => json_ok(status),
        Err(e) => json_err(500, &e.to_string()),
    }
}
async fn health(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    match store.health_stats().await { Ok(s) => json_ok(json!({"status": "ok", "stats": s})), Err(e) => json_err(500, &e.to_string()) }
}

async fn dashboard(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    match (store.health_stats().await, store.feed_stats().await) {
        (Ok(stats), Ok(feeds)) => json_ok(json!({"status": "ok", "stats": stats, "feeds": feeds})),
        _ => json_err(500, "dashboard query failed"),
    }
}

async fn stats(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    match (store.score_distribution().await, store.article_trend(14).await) {
        (Ok(scores), Ok(trend)) => json_ok(json!({"score_distribution": scores, "articles_per_day": trend})),
        _ => json_err(500, "stats query failed"),
    }
}

async fn categories(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cache_key = "v1:categories";
    if let Some(cached) = cache_get(&ctx.env, cache_key).await {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cached) {
            let mut resp = Response::from_json(&v)?;
            cors_headers(&mut resp);
            return Ok(resp);
        }
    }
    let store = Store::new(ctx.env.d1("DB")?);
    match store.categories_summary().await {
        Ok(list) => {
            let result = serde_json::json!({"categories": list.into_iter().map(|(cat, count)| serde_json::json!({"category": cat, "article_count": count})).collect::<Vec<_>>()});
            if let Ok(json_str) = serde_json::to_string(&result) {
                cache_put(&ctx.env, cache_key, &json_str, 120).await;
            }
            json_ok(result)
        }
        Err(e) => json_err(500, &e.to_string()),
    }
}


async fn intelligence_signals(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let now = (js_sys::Date::now() / 1000.0) as i64;
    match store.signals_today(now).await {
        Ok(signals) => json_ok(json!({
            "date": fmt_date_ymd(now),
            "generated_at": fmt_datetime_iso(now),
            "signals": signals,
        })),
        Err(e) => json_err(500, &e.to_string()),
    }
}
async fn tags(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cache_key = "v1:tags";
    if let Some(cached) = cache_get(&ctx.env, cache_key).await {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cached) {
            let mut resp = Response::from_json(&v)?;
            cors_headers(&mut resp);
            return Ok(resp);
        }
    }
    let store = Store::new(ctx.env.d1("DB")?);
    match store.tags_summary().await {
        Ok(list) => {
            let result = serde_json::json!({"tags": list.into_iter().map(|(tag, count)| serde_json::json!({"tag": tag, "count": count})).collect::<Vec<_>>()});
            if let Ok(json_str) = serde_json::to_string(&result) {
                cache_put(&ctx.env, cache_key, &json_str, 120).await;
            }
            json_ok(result)
        }
        Err(e) => json_err(500, &e.to_string()),
    }
}


async fn feeds_list(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let status_filter = req.url().ok().and_then(|u| u.query_pairs().find(|(k, _)| k == "status").map(|(_, v)| v.to_string()));
    match store.all_feeds(status_filter.as_deref()).await { Ok(list) => json_ok(json!({"feeds": list})), Err(e) => json_err(500, &e.to_string()) }
}

async fn feeds_get(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") { Some(v) => v, None => return json_err(400, "invalid id") };
    match store.get_feed(id).await { Ok(Some(feed)) => json_ok(json!({"feed": feed})), Ok(None) => json_err(404, "feed not found"), Err(e) => json_err(500, &e.to_string()) }
}

#[derive(Deserialize)] struct CreateFeedBody { url: String, title: Option<String>, category: Option<String>, fetch_interval_sec: Option<i64> }

async fn feeds_create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let body: CreateFeedBody = match req.json().await { Ok(b) => b, Err(_) => return json_err(400, "invalid JSON body") };
    if body.url.is_empty() { return json_err(400, "url is required"); }
    match store.insert_feed(&body.url, body.title.as_deref().unwrap_or("Untitled"), body.category.as_deref().unwrap_or("uncategorized"), body.fetch_interval_sec.unwrap_or(3600)).await {
        Ok(Some(id)) => match store.get_feed(id).await { Ok(Some(feed)) => json_ok(json!({"feed": feed})), _ => json_ok(json!({"id": id})) },
        Ok(None) => json_err(409, "feed with this URL already exists"), Err(e) => json_err(500, &e.to_string()),
    }
}

#[derive(Deserialize)] struct UpdateFeedBody { title: Option<String>, category: Option<String>, fetch_interval_sec: Option<i64>, extraction_level: Option<String>, status: Option<String> }

async fn feeds_update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") { Some(v) => v, None => return json_err(400, "invalid id") };
    let body: UpdateFeedBody = match req.json().await { Ok(b) => b, Err(_) => return json_err(400, "invalid JSON body") };
    if let Some(ref status) = body.status { if let Err(e) = store.set_feed_status(id, status).await { return json_err(500, &e.to_string()); } }
    if let Err(e) = store.update_feed(id, body.title.as_deref(), body.category.as_deref(), body.fetch_interval_sec, body.extraction_level.as_deref()).await { return json_err(500, &e.to_string()); }
    match store.get_feed(id).await { Ok(Some(feed)) => json_ok(json!({"feed": feed})), Ok(None) => json_err(404, "feed not found"), Err(e) => json_err(500, &e.to_string()) }
}

async fn feeds_delete(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") { Some(v) => v, None => return json_err(400, "invalid id") };
    match store.set_feed_status(id, "inactive").await { Ok(()) => json_ok(json!({"status": "deleted", "id": id})), Err(e) => json_err(500, &e.to_string()) }
}

async fn trending(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let url = req.url()?;
    let limit = parse_limit(&url);
    let offset = parse_offset(&url);
    let total = store.trending_count().await.unwrap_or(0);
    match store.trending_articles(limit, offset).await {
        Ok(articles) => json_ok(json!({"articles": articles, "total": total, "limit": limit, "offset": offset})),
        Err(e) => json_err(500, &e.to_string()),
    }
}


async fn article_content(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") { Some(v) => v, None => return json_err(400, "missing id") };
    match store.get_raw_content_key(id).await {
        Ok(Some(k)) => {
            let bucket = match ctx.env.bucket("RAW_CONTENT") {
                Ok(b) => b,
                Err(e) => return json_err(500, &format!("RAW_CONTENT bucket: {e}")),
            };
            match bucket.get(&k).execute().await {
                Ok(Some(obj)) => match obj.body() {
                    Some(body) => match body.text().await {
                        Ok(t) => json_ok(json!({"id": id, "content": t, "format": "html", "source": "r2"})),
                        Err(e) => json_err(500, &format!("body read: {e}")),
                    },
                    None => json_err(500, "R2 object has no body"),
                },
                Ok(None) => json_err(404, "content not found in storage"),
                Err(e) => json_err(500, &format!("R2 read: {e}")),
            }
        }
        Ok(None) => json_err(404, "no raw content for this article"),
        Err(e) => json_err(500, &e.to_string()),
    }
}




async fn article_detail(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") { Some(v) => v, None => return json_err(400, "missing id") };
    match store.article_detail(id).await { Ok(Some(a)) => json_ok(json!({"article": a})), Ok(None) => json_err(404, "not found"), Err(e) => json_err(500, &e.to_string()) }
}

async fn articles_batch(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let ids_param = req.url().ok().and_then(|u| u.query_pairs().find(|(k, _)| k == "ids").map(|(_, v)| v.to_string())).unwrap_or_default();
    let ids: Vec<i64> = ids_param.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    if ids.is_empty() {
        return json_err(400, "missing or empty 'ids' query parameter 鈥?expected comma-separated integers");
    }
    match store.articles_by_ids(&ids).await {
        Ok(articles) => json_ok(json!({"articles": articles})),
        Err(e) => json_err(500, &e.to_string()),
    }
}

async fn article_adjacent(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") { Some(v) => v, None => return json_err(400, "missing id") };
    match store.adjacent_articles(id).await {
        Ok((prev, next)) => json_ok(json!({"prev": prev, "next": next})),
        Err(e) => json_err(500, &e.to_string()),
    }
}

async fn article_related(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") { Some(v) => v, None => return json_err(400, "missing id") };
    match store.related_articles(id, 6).await { Ok(articles) => json_ok(json!({"articles": articles})), Err(e) => json_err(500, &e.to_string()) }
}

async fn latest_articles(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let url = req.url()?;
    let tag: Option<String> = url.query_pairs().find(|(k, _)| k == "tag").map(|(_, v)| v.to_string());
    let category: Option<String> = url.query_pairs().find(|(k, _)| k == "category").map(|(_, v)| v.to_string());
    let limit = parse_limit(&url);
    let offset = parse_offset(&url);
    if tag.is_none() && category.is_none() && limit == 30 && offset == 0 {
        let cache_key = "v1:latest:30:0";
        if let Some(cached) = cache_get(&ctx.env, cache_key).await {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cached) {
                let mut resp = Response::from_json(&v)?;
                cors_headers(&mut resp);
                return Ok(resp);
            }
        }
        let store = Store::new(ctx.env.d1("DB")?);
        let total = store.article_count().await.unwrap_or(0);
        match store.latest_articles(30, 0).await {
            Ok(a) => {
                let result = serde_json::json!({"articles": a, "total": total, "limit": 30, "offset": 0});
                if let Ok(json_str) = serde_json::to_string(&result) {
                    cache_put(&ctx.env, cache_key, &json_str, 60).await;
                }
                json_ok(result)
            }
            Err(e) => json_err(500, &e.to_string()),
        }
    } else {
        let store = Store::new(ctx.env.d1("DB")?);
        if let Some(ref tag) = tag {
            return match store.articles_by_tag(tag, limit, offset).await {
                Ok(a) => json_ok(json!({"articles": a, "limit": limit, "offset": offset})),
                Err(e) => json_err(500, &e.to_string()),
            };
        }
        if let Some(ref cat) = category {
            return match store.articles_by_category(cat, limit, offset).await {
                Ok(a) => json_ok(json!({"articles": a, "limit": limit, "offset": offset})),
                Err(e) => json_err(500, &e.to_string()),
            };
        }
        let total = store.article_count().await.unwrap_or(0);
        match store.latest_articles(limit, offset).await {
            Ok(a) => json_ok(json!({"articles": a, "total": total, "limit": limit, "offset": offset})),
            Err(e) => json_err(500, &e.to_string()),
        }
    }
}


async fn search_articles(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let db = ctx.env.d1("DB")?;
    let search = D1FtsSearch::new(&db);
    let url = req.url()?;
    let query: String = url.query_pairs().find(|(k, _)| k == "q").map(|(_, v)| v.to_string()).unwrap_or_default();
    if query.is_empty() { return json_err(400, "missing query parameter 'q'"); }
    let tag: Option<String> = url.query_pairs().find(|(k, _)| k == "tag").map(|(_, v)| v.to_string());
    let category: Option<String> = url.query_pairs().find(|(k, _)| k == "category").map(|(_, v)| v.to_string());
    let sort: Option<String> = url.query_pairs().find(|(k, _)| k == "sort").map(|(_, v)| v.to_string());
    let limit = parse_limit(&url);
    let offset = parse_offset(&url);

    let total = search
        .search_count(&query, tag.as_deref(), category.as_deref())
        .await
        .unwrap_or(0);

    match search.search_filtered(&query, limit, offset, tag.as_deref(), category.as_deref(), sort.as_deref()).await {
        Ok(hits) => json_ok(json!({"results": hits, "total": total, "limit": limit, "offset": offset})),
        Err(e) => json_err(500, &e.to_string())
    }
}

// ---- Rules CRUD handlers ----

async fn rules_list(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    match store.list_rules().await {
        Ok(list) => json_ok(json!({"rules": list})),
        Err(e) => json_err(500, &e.to_string()),
    }
}

async fn rules_get(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") { Some(v) => v, None => return json_err(400, "invalid id") };
    match store.get_rule(id).await {
        Ok(Some(rule)) => json_ok(json!({"rule": rule})),
        Ok(None) => json_err(404, "rule not found"),
        Err(e) => json_err(500, &e.to_string()),
    }
}

#[derive(Deserialize)]
struct CreateRuleBody {
    name: String,
    rule_json: String,  // condition-only JSON from frontend
    audience_tag: Option<String>,
    signal_type: Option<String>,
    score_delta: Option<f64>,
}

async fn rules_create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let body: CreateRuleBody = match req.json().await { Ok(b) => b, Err(_) => return json_err(400, "invalid JSON body") };
    if body.name.is_empty() { return json_err(400, "name is required"); }
    if body.rule_json.is_empty() { return json_err(400, "rule_json is required"); }

    // Validate condition JSON
    if let Err(e) = serde_json::from_str::<serde_json::Value>(&body.rule_json) {
        return json_err(400, &format!("invalid condition JSON: {e}"));
    }

    // Reconstruct full Rule JSON for the scoring pipeline (active_rule_jsons 鈫?rules::score
    // expects {name, audience_tag, condition, score_delta}).
    let full_rule = serde_json::json!({
        "name": body.name,
        "audience_tag": body.audience_tag.clone().unwrap_or_else(|| "default".into()),
        "condition": serde_json::from_str::<serde_json::Value>(&body.rule_json).unwrap(),
        "score_delta": body.score_delta.unwrap_or(0.0),
    });
    let full_rule_str = serde_json::to_string(&full_rule).unwrap_or(body.rule_json.clone());

    match store.insert_rule(
        &body.name,
        &full_rule_str,
        &body.audience_tag.unwrap_or_else(|| "default".into()),
        body.signal_type.as_deref(),
        body.score_delta.unwrap_or(0.0),
    ).await {
        Ok(Some(id)) => match store.get_rule(id).await { Ok(Some(rule)) => json_ok(json!({"rule": rule})), _ => json_ok(json!({"id": id})) },
        Ok(None) => json_err(500, "rule creation returned no id"),
        Err(e) => json_err(500, &e.to_string()),
    }
}

#[derive(Deserialize)]
struct UpdateRuleBody {
    name: Option<String>,
    rule_json: Option<String>,       // condition-only JSON from frontend
    enabled: Option<bool>,
    signal_type: Option<Option<String>>, // None = not sent, Some(None) = clear, Some(Some(v)) = set
}

async fn rules_update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") { Some(v) => v, None => return json_err(400, "invalid id") };
    let body: UpdateRuleBody = match req.json().await { Ok(b) => b, Err(_) => return json_err(400, "invalid JSON body") };

    // If rule_json is being updated, wrap condition-only JSON in full Rule JSON
    let mut rule_json_for_store: Option<String> = None;
    if let Some(ref cond_json) = body.rule_json {
        if let Ok(Some(existing)) = store.get_rule(id).await {
            let full_rule = serde_json::json!({
                "name": body.name.as_deref().unwrap_or(&existing.name),
                "audience_tag": existing.audience_tag,
                "condition": serde_json::from_str::<serde_json::Value>(cond_json).unwrap_or_default(),
                "score_delta": existing.score_delta,
            });
            rule_json_for_store = Some(serde_json::to_string(&full_rule).unwrap_or_else(|_| cond_json.clone()));
        } else {
            return json_err(404, "rule not found for update");
        }
    }

    if let Err(e) = store.update_rule(
        id,
        body.name.as_deref(),
        rule_json_for_store.as_deref().or(body.rule_json.as_deref()),
        body.enabled,
        body.signal_type.as_ref().map(|opt| opt.as_deref()),
    ).await {
        return json_err(500, &e.to_string());
    }
    match store.get_rule(id).await { Ok(Some(rule)) => json_ok(json!({"rule": rule})), Ok(None) => json_err(404, "rule not found"), Err(e) => json_err(500, &e.to_string()) }
}

async fn rules_delete(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") { Some(v) => v, None => return json_err(400, "invalid id") };
    match store.delete_rule(id).await {
        Ok(()) => json_ok(json!({"status": "disabled", "id": id})),
        Err(e) => json_err(500, &e.to_string()),
    }
}



