//! HTTP routes, built with the `worker` crate's built-in router.

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
        .get_async("/api/tags", tags)
        .get_async("/api/articles/latest", latest_articles)
        .get_async("/api/articles/search", search_articles)
}

async fn health(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    match store.health_stats().await {
        Ok(stats) => Response::from_json(&json!({ "status": "ok", "stats": stats })),
        Err(e) => Response::error(e.to_string(), 500),
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
            Response::from_json(&json!({ "tags": tags }))
        }
        Err(e) => Response::error(e.to_string(), 500),
    }
}

async fn latest_articles(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let url = req.url()?;

    // If ?tag= is present, filter by tag instead of returning latest
    let tag: Option<String> = url
        .query_pairs()
        .find(|(k, _)| k == "tag")
        .map(|(_, v)| v.to_string());

    if let Some(ref tag) = tag {
        let limit = parse_limit(&url);
        return match store.articles_by_tag(tag, limit).await {
            Ok(articles) => Response::from_json(&json!({ "articles": articles })),
            Err(e) => Response::error(e.to_string(), 500),
        };
    }

    let limit = parse_limit(&url);
    match store.latest_articles(limit).await {
        Ok(articles) => Response::from_json(&json!({ "articles": articles })),
        Err(e) => Response::error(e.to_string(), 500),
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

    let limit = parse_limit(&url);

    if query.is_empty() {
        return Response::error("missing query parameter 'q'", 400);
    }

    match search.search(&query, limit).await {
        Ok(hits) => Response::from_json(&json!({ "results": hits })),
        Err(e) => Response::error(e.to_string(), 500),
    }
}
