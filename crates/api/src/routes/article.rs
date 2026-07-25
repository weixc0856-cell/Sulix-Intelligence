use serde_json::json;
use worker::*;

use search::D1FtsSearch;
use store::Store;

use crate::shared::{params, response};

pub(crate) async fn latest_articles(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let url = req.url()?;
    let tag: Option<String> = url.query_pairs().find(|(k, _)| k == "tag").map(|(_, v)| v.to_string());
    let category: Option<String> = url.query_pairs().find(|(k, _)| k == "category").map(|(_, v)| v.to_string());
    let limit = params::parse_limit(&url);
    let offset = params::parse_offset(&url);
    if tag.is_none() && category.is_none() && limit == 30 && offset == 0 {
        let cache_key = "v1:latest:30:0";
        if let Some(cached) = crate::cache_get(&ctx.env, cache_key).await {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cached) {
                let mut resp = Response::from_json(&v)?;
                response::cors_headers(&mut resp);
                return Ok(resp);
            }
        }
        let store = Store::new(ctx.env.d1("DB")?);
        let total = store.article_count().await.unwrap_or(0);
        match store.latest_articles(30, 0).await {
            Ok(a) => {
                let result = serde_json::json!({"articles": a, "total": total, "limit": 30, "offset": 0});
                if let Ok(json_str) = serde_json::to_string(&result) {
                    crate::cache_put(&ctx.env, cache_key, &json_str, 60).await;
                }
                response::json_ok(result)
            }
            Err(e) => response::json_err_internal(&e.to_string()),
        }
    } else {
        let store = Store::new(ctx.env.d1("DB")?);
        if let Some(ref tag) = tag {
            return match store.articles_by_tag(tag, limit, offset).await {
                Ok(a) => response::json_ok(json!({"articles": a, "limit": limit, "offset": offset})),
                Err(e) => response::json_err_internal(&e.to_string()),
            };
        }
        if let Some(ref cat) = category {
            return match store.articles_by_category(cat, limit, offset).await {
                Ok(a) => response::json_ok(json!({"articles": a, "limit": limit, "offset": offset})),
                Err(e) => response::json_err_internal(&e.to_string()),
            };
        }
        let total = store.article_count().await.unwrap_or(0);
        match store.latest_articles(limit, offset).await {
            Ok(a) => response::json_ok(json!({"articles": a, "total": total, "limit": limit, "offset": offset})),
            Err(e) => response::json_err_internal(&e.to_string()),
        }
    }
}

pub(crate) async fn search_articles(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let db = ctx.env.d1("DB")?;
    let search = D1FtsSearch::new(&db);
    let url = req.url()?;
    let query: String = url.query_pairs().find(|(k, _)| k == "q").map(|(_, v)| v.to_string()).unwrap_or_default();
    if query.is_empty() {
        return response::json_err(400, "missing query parameter 'q'");
    }
    let tag: Option<String> = url.query_pairs().find(|(k, _)| k == "tag").map(|(_, v)| v.to_string());
    let category: Option<String> = url.query_pairs().find(|(k, _)| k == "category").map(|(_, v)| v.to_string());
    let sort: Option<String> = url.query_pairs().find(|(k, _)| k == "sort").map(|(_, v)| v.to_string());
    let limit = params::parse_limit(&url);
    let offset = params::parse_offset(&url);

    let total = search.search_count(&query, tag.as_deref(), category.as_deref()).await.unwrap_or(0);

    match search.search_filtered(&query, limit, offset, tag.as_deref(), category.as_deref(), sort.as_deref()).await {
        Ok(hits) => response::json_ok(json!({"results": hits, "total": total, "limit": limit, "offset": offset})),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn article_detail(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match params::param_i64(&ctx, "id") {
        Some(v) => v,
        None => return response::json_err(400, "missing id"),
    };
    match store.article_detail(id).await {
        Ok(Some(a)) => response::json_ok(json!({"article": a})),
        Ok(None) => response::json_err(404, "not found"),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn article_content(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match params::param_i64(&ctx, "id") {
        Some(v) => v,
        None => return response::json_err(400, "missing id"),
    };
    match store.get_raw_content_key(id).await {
        Ok(Some(k)) => {
            let bucket = match ctx.env.bucket("RAW_CONTENT") {
                Ok(b) => b,
                Err(e) => return response::json_err_internal(&format!("RAW_CONTENT bucket: {e}")),
            };
            match bucket.get(&k).execute().await {
                Ok(Some(obj)) => match obj.body() {
                    Some(body) => match body.text().await {
                        Ok(t) => response::json_ok(json!({"id": id, "content": t, "format": "html", "source": "r2"})),
                        Err(e) => response::json_err_internal(&format!("body read: {e}")),
                    },
                    None => response::json_err(500, "R2 object has no body"),
                },
                Ok(None) => response::json_err(404, "content not found in storage"),
                Err(e) => response::json_err_internal(&format!("R2 read: {e}")),
            }
        }
        Ok(None) => response::json_err(404, "no raw content for this article"),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn articles_batch(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let ids_param = req
        .url()
        .ok()
        .and_then(|u| u.query_pairs().find(|(k, _)| k == "ids").map(|(_, v)| v.to_string()))
        .unwrap_or_default();
    let ids: Vec<i64> = ids_param.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    if ids.is_empty() {
        return response::json_err(400, "missing or empty ids query parameter - expected comma-separated integers");
    }
    match store.articles_by_ids(&ids).await {
        Ok(articles) => response::json_ok(json!({"articles": articles})),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn article_adjacent(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match params::param_i64(&ctx, "id") {
        Some(v) => v,
        None => return response::json_err(400, "missing id"),
    };
    match store.adjacent_articles(id).await {
        Ok((prev, next)) => response::json_ok(json!({"prev": prev, "next": next})),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn article_related(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match params::param_i64(&ctx, "id") {
        Some(v) => v,
        None => return response::json_err(400, "missing id"),
    };
    match store.related_articles(id, 6).await {
        Ok(articles) => response::json_ok(json!({"articles": articles})),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn trending(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let url = req.url()?;
    let limit = params::parse_limit(&url);
    let offset = params::parse_offset(&url);
    let total = store.trending_count().await.unwrap_or(0);
    match store.trending_articles(limit, offset).await {
        Ok(articles) => {
            response::json_ok(json!({"articles": articles, "total": total, "limit": limit, "offset": offset}))
        }
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}
