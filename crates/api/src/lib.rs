//! HTTP routes, built with the `worker` crate's built-in router.

use serde::Deserialize;
use serde_json::json;
use worker::*;

use search::{ArticleSearch, D1FtsSearch};
use store::Store;

fn parse_limit(url: &Url) -> u32 {
    url.query_pairs()
        .find(|(k, _)| k == "limit")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(30)
}

pub fn router() -> Router<'static, ()> {
    Router::new()
        .get_async("/api/health", health)
        .get_async("/api/debug/feeds-due", debug_feeds_due)
        .get_async("/api/dashboard", dashboard)
        .get_async("/api/stats", stats)
        .get_async("/api/categories", categories)
        .get_async("/api/tags", tags)
        .get_async("/api/feeds", feeds_list)
        .post_async("/api/feeds", feeds_create)
        .get_async("/api/feeds/:id", feeds_get)
        .put_async("/api/feeds/:id", feeds_update)
        .delete_async("/api/feeds/:id", feeds_delete)
        .get_async("/api/articles/latest", latest_articles)
        .get_async("/api/articles/trending", trending)
        .get_async("/api/articles/search", search_articles)
        .get_async("/api/articles/:id/related", article_related)
        .get_async("/api/articles/:id", article_detail)
}

// ---- Helpers ----

fn json_ok(v: serde_json::Value) -> Result<Response> {
    Response::from_json(&v)
}

fn json_err(status: u16, msg: &str) -> Result<Response> {
    Response::error(msg, status)
}

fn param_i64(ctx: &RouteContext<()>, name: &str) -> Option<i64> {
    ctx.param(name)?.parse().ok()
}

// ---- Health ----

async fn debug_feeds_due(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let now = (js_sys::Date::now() / 1000.0) as i64;
    match store.feeds_due_for_fetch(now, None).await {
        Ok(feeds) => json_ok(json!({
            "now": now,
            "feeds_due": feeds.len(),
            "feeds": feeds.iter().map(|f| json!({
                "id": f.id, "title": f.title, "last_fetched_at": f.last_fetched_at,
                "fetch_interval_sec": f.fetch_interval_sec, "extraction_level": f.extraction_level
            })).collect::<Vec<_>>()
        })),
        Err(e) => json_err(500, &e.to_string()),
    }
}

async fn health(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    match store.health_stats().await {
        Ok(stats) => json_ok(json!({ "status": "ok", "stats": stats })),
        Err(e) => json_err(500, &e.to_string()),
    }
}

// ---- Dashboard ----

async fn dashboard(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    match (store.health_stats().await, store.feed_stats().await) {
        (Ok(stats), Ok(feed_list)) => json_ok(json!({
            "status": "ok",
            "stats": stats,
            "feeds": feed_list,
        })),
        _ => json_err(500, "dashboard query failed"),
    }
}

// ---- Tags ----

async fn stats(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    match (store.score_distribution().await, store.article_trend(14).await) {
        (Ok(scores), Ok(trend)) => json_ok(json!({
            "score_distribution": scores,
            "articles_per_day": trend,
        })),
        _ => json_err(500, "stats query failed"),
    }
}

async fn categories(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    match store.categories_summary().await {
        Ok(list) => {
            let cats: Vec<serde_json::Value> = list
                .into_iter()
                .map(|(cat, count)| json!({ "category": cat, "article_count": count }))
                .collect();
            json_ok(json!({ "categories": cats }))
        }
        Err(e) => json_err(500, &e.to_string()),
    }
}

async fn tags(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    match store.tags_summary().await {
        Ok(list) => {
            let tags: Vec<serde_json::Value> = list
                .into_iter()
                .map(|(tag, count)| json!({ "tag": tag, "count": count }))
                .collect();
            json_ok(json!({ "tags": tags }))
        }
        Err(e) => json_err(500, &e.to_string()),
    }
}

// ---- Feed CRUD ----

async fn feeds_list(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let status_filter = req.url().ok()
        .and_then(|u| u.query_pairs().find(|(k, _)| k == "status").map(|(_, v)| v.to_string()));
    match store.all_feeds(status_filter.as_deref()).await {
        Ok(list) => json_ok(json!({ "feeds": list })),
        Err(e) => json_err(500, &e.to_string()),
    }
}

async fn feeds_get(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") {
        Some(v) => v,
        None => return json_err(400, "invalid id"),
    };
    match store.get_feed(id).await {
        Ok(Some(feed)) => json_ok(json!({ "feed": feed })),
        Ok(None) => json_err(404, "feed not found"),
        Err(e) => json_err(500, &e.to_string()),
    }
}

#[derive(Deserialize)]
struct CreateFeedBody {
    url: String,
    title: Option<String>,
    category: Option<String>,
    fetch_interval_sec: Option<i64>,
}

async fn feeds_create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);

    let body: CreateFeedBody = match req.json().await {
        Ok(b) => b,
        Err(_) => return json_err(400, "invalid JSON body"),
    };

    if body.url.is_empty() {
        return json_err(400, "url is required");
    }

    let title = body.title.as_deref().unwrap_or("Untitled");
    let category = body.category.as_deref().unwrap_or("uncategorized");
    let interval = body.fetch_interval_sec.unwrap_or(3600);

    match store.insert_feed(&body.url, title, category, interval).await {
        Ok(Some(id)) => {
            // Fetch back the full row.
            match store.get_feed(id).await {
                Ok(Some(feed)) => json_ok(json!({ "feed": feed })),
                _ => json_ok(json!({ "id": id })),
            }
        }
        Ok(None) => json_err(409, "feed with this URL already exists"),
        Err(e) => json_err(500, &e.to_string()),
    }
}

#[derive(Deserialize)]
struct UpdateFeedBody {
    title: Option<String>,
    category: Option<String>,
    fetch_interval_sec: Option<i64>,
    extraction_level: Option<String>,
    status: Option<String>,
}

async fn feeds_update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") {
        Some(v) => v,
        None => return json_err(400, "invalid id"),
    };

    let body: UpdateFeedBody = match req.json().await {
        Ok(b) => b,
        Err(_) => return json_err(400, "invalid JSON body"),
    };

    // Apply status change separately if provided
    if let Some(ref status) = body.status {
        if let Err(e) = store.set_feed_status(id, status).await {
            return json_err(500, &e.to_string());
        }
    }

    // Apply field updates
    if let Err(e) = store.update_feed(
        id,
        body.title.as_deref(),
        body.category.as_deref(),
        body.fetch_interval_sec,
        body.extraction_level.as_deref(),
    ).await {
        return json_err(500, &e.to_string());
    }

    // Return updated feed
    match store.get_feed(id).await {
        Ok(Some(feed)) => json_ok(json!({ "feed": feed })),
        Ok(None) => json_err(404, "feed not found"),
        Err(e) => json_err(500, &e.to_string()),
    }
}

async fn feeds_delete(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") {
        Some(v) => v,
        None => return json_err(400, "invalid id"),
    };

    // Soft-delete: set status to inactive.
    if let Err(e) = store.set_feed_status(id, "inactive").await {
        return json_err(500, &e.to_string());
    }

    json_ok(json!({ "status": "deleted", "id": id }))
}

// ---- Articles ----

async fn trending(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let limit = 50;
    match store.trending_articles(limit).await {
        Ok(articles) => json_ok(json!({ "articles": articles })),
        Err(e) => json_err(500, &e.to_string()),
    }
}

async fn article_detail(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") {
        Some(v) => v,
        None => return json_err(400, "missing id"),
    };
    match store.article_detail(id).await {
        Ok(Some(article)) => json_ok(json!({ "article": article })),
        Ok(None) => json_err(404, "not found"),
        Err(e) => json_err(500, &e.to_string()),
    }
}

async fn article_related(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") {
        Some(v) => v,
        None => return json_err(400, "missing id"),
    };
    // Default to 6 related articles (2 rows of 3 on desktop)
    let limit = 6;
    match store.related_articles(id, limit).await {
        Ok(articles) => json_ok(json!({ "articles": articles })),
        Err(e) => json_err(500, &e.to_string()),
    }
}

async fn latest_articles(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let request_url = req.url()?;

    let tag: Option<String> = request_url
        .query_pairs()
        .find(|(k, _)| k == "tag")
        .map(|(_, v)| v.to_string());
    let category: Option<String> = request_url
        .query_pairs()
        .find(|(k, _)| k == "category")
        .map(|(_, v)| v.to_string());

    if let Some(ref tag) = tag {
        let limit = parse_limit(&request_url);
        return match store.articles_by_tag(tag, limit).await {
            Ok(articles) => json_ok(json!({ "articles": articles })),
            Err(e) => json_err(500, &e.to_string()),
        };
    }

    if let Some(ref cat) = category {
        let limit = parse_limit(&request_url);
        return match store.articles_by_category(cat, limit).await {
            Ok(articles) => json_ok(json!({ "articles": articles })),
            Err(e) => json_err(500, &e.to_string()),
        };
    }

    let limit = parse_limit(&request_url);
    match store.latest_articles(limit).await {
        Ok(articles) => json_ok(json!({ "articles": articles })),
        Err(e) => json_err(500, &e.to_string()),
    }
}

async fn search_articles(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let db = ctx.env.d1("DB")?;
    let search = D1FtsSearch::new(&db);

    let url = req.url()?;
    let query: String = url
        .query_pairs()
        .find(|(k, _)| k == "q")
        .map(|(_, v)| v.to_string())
        .unwrap_or_default();
    let tag: Option<String> = url.query_pairs().find(|(k, _)| k == "tag").map(|(_, v)| v.to_string());
    let category: Option<String> = url.query_pairs().find(|(k, _)| k == "category").map(|(_, v)| v.to_string());
    let sort: Option<String> = url.query_pairs().find(|(k, _)| k == "sort").map(|(_, v)| v.to_string());
    let limit = parse_limit(&url);

    if query.is_empty() {
        return json_err(400, "missing query parameter 'q'");
    }

    match search.search_filtered(&query, limit, tag.as_deref(), category.as_deref(), sort.as_deref()).await {
        Ok(hits) => json_ok(json!({ "results": hits })),
        Err(e) => json_err(500, &e.to_string()),
    }
}
