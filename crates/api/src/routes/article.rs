use application::ArticleService;
use serde_json::json;
use worker::*;

use store::Store;

use crate::shared::{params, response};

pub(crate) async fn latest_articles(req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let url = req.url()?;
    let tag: Option<String> = url.query_pairs().find(|(k, _)| k == "tag").map(|(_, v)| v.to_string());
    let category: Option<String> = url.query_pairs().find(|(k, _)| k == "category").map(|(_, v)| v.to_string());
    let limit = params::parse_limit(&url);
    let offset = params::parse_offset(&url);
    let service = ArticleService::new(ctx.data.clone());
    if tag.is_none() && category.is_none() && limit == 30 && offset == 0 {
        let cache_key = "v1:latest:30:0";
        if let Some(cached) = crate::cache_get(&ctx.env, cache_key).await {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cached) {
                let mut resp = Response::from_json(&v)?;
                response::cors_headers(&mut resp);
                return Ok(resp);
            }
        }
        let total = service.count().await.unwrap_or(0);
        match service.latest(30, 0).await {
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
        if let Some(ref tag) = tag {
            return match service.by_tag(tag, limit, offset).await {
                Ok(a) => response::json_ok(json!({"articles": a, "limit": limit, "offset": offset})),
                Err(e) => response::json_err_internal(&e.to_string()),
            };
        }
        if let Some(ref cat) = category {
            return match service.by_category(cat, limit, offset).await {
                Ok(a) => response::json_ok(json!({"articles": a, "limit": limit, "offset": offset})),
                Err(e) => response::json_err_internal(&e.to_string()),
            };
        }
        let total = service.count().await.unwrap_or(0);
        match service.latest(limit, offset).await {
            Ok(a) => response::json_ok(json!({"articles": a, "total": total, "limit": limit, "offset": offset})),
            Err(e) => response::json_err_internal(&e.to_string()),
        }
    }
}

pub(crate) async fn article_detail(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = ArticleService::new(ctx.data.clone());
    let id = match params::param_i64(&ctx, "id") {
        Some(v) => v,
        None => return response::json_err(400, "missing id"),
    };
    match service.detail(id).await {
        Ok(Some((article, provenance))) => response::json_ok(json!({
            "article": article,
            "provenance": provenance,
        })),
        Ok(None) => response::json_err(404, "not found"),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn article_content(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let store = ctx.data.clone();
    let id = match params::param_i64(&ctx, "id") {
        Some(v) => v,
        None => return response::json_err(400, "missing id"),
    };

    // Resolve source and check policy before serving content
    if let Ok(Some(article)) = store.article_by_id(id).await {
        if let Ok(Some(source)) = store.find_source_by_feed(article.feed_id).await {
            let decision = content_governance::evaluate_policy(&source);
            if decision.serving == content_governance::ServingPermission::Denied {
                return response::json_err(403, "Content access denied by source policy");
            }
        }
    }

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

pub(crate) async fn articles_batch(req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = ArticleService::new(ctx.data.clone());
    let ids_param = req
        .url()
        .ok()
        .and_then(|u| u.query_pairs().find(|(k, _)| k == "ids").map(|(_, v)| v.to_string()))
        .unwrap_or_default();
    let ids: Vec<i64> = ids_param.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    if ids.is_empty() {
        return response::json_err(400, "missing or empty ids query parameter - expected comma-separated integers");
    }
    match service.batch(&ids).await {
        Ok(articles) => response::json_ok(json!({"articles": articles})),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn article_adjacent(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = ArticleService::new(ctx.data.clone());
    let id = match params::param_i64(&ctx, "id") {
        Some(v) => v,
        None => return response::json_err(400, "missing id"),
    };
    match service.adjacent(id).await {
        Ok((prev, next)) => response::json_ok(json!({"prev": prev, "next": next})),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn article_related(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = ArticleService::new(ctx.data.clone());
    let id = match params::param_i64(&ctx, "id") {
        Some(v) => v,
        None => return response::json_err(400, "missing id"),
    };
    match service.related(id, 6).await {
        Ok(articles) => response::json_ok(json!({"articles": articles})),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn trending(req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = ArticleService::new(ctx.data.clone());
    let url = req.url()?;
    let limit = params::parse_limit(&url);
    let offset = params::parse_offset(&url);
    let total = service.trending_count().await.unwrap_or(0);
    match service.trending(limit, offset).await {
        Ok(articles) => {
            response::json_ok(json!({"articles": articles, "total": total, "limit": limit, "offset": offset}))
        }
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}
