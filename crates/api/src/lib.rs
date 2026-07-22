//! HTTP routes, built with the `worker` crate's built-in router.
//! No auth middleware yet (free-tier launch) -- routes are grouped under
//! `router()` precisely so middleware can be added later without moving
//! anything around.

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
        .get_async("/api/articles/latest", latest_articles)
        .get_async("/api/articles/search", search_articles)
}

async fn latest_articles(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let db = ctx.env.d1("DB")?;
    let store = Store::new(db);

    let limit = req.url().ok().map(|u| parse_limit(&u)).unwrap_or(30);

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
