//! Feed CRUD API handlers.
//!
//! Routes:
//! - `GET    /api/feeds`       — list all feeds (optional ?status= filter)
//! - `GET    /api/feeds/:id`   — get feed by id
//! - `POST   /api/feeds`       — create a new feed
//! - `PUT    /api/feeds/:id`   — update an existing feed
//! - `DELETE /api/feeds/:id`   — soft-delete (set status to "inactive")

use serde::Deserialize;
use serde_json::json;
use worker::*;

use store::Store;

use crate::shared::{params, response};

pub(crate) async fn feeds_list(req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let store = ctx.data.clone();
    let status_filter =
        req.url().ok().and_then(|u| u.query_pairs().find(|(k, _)| k == "status").map(|(_, v)| v.to_string()));
    match store.all_feeds(status_filter.as_deref()).await {
        Ok(list) => response::json_ok(json!({"feeds": list})),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn feeds_get(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let store = ctx.data.clone();
    let id = match params::param_i64(&ctx, "id") {
        Some(v) => v,
        None => return response::json_err(400, "invalid id"),
    };
    match store.get_feed(id).await {
        Ok(Some(feed)) => response::json_ok(json!({"feed": feed})),
        Ok(None) => response::json_err(404, "feed not found"),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

#[derive(Deserialize)]
struct CreateFeedBody {
    url: String,
    title: Option<String>,
    category: Option<String>,
    fetch_interval_sec: Option<i64>,
}

pub(crate) async fn feeds_create(mut req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let store = ctx.data.clone();
    let body: CreateFeedBody = match req.json().await {
        Ok(b) => b,
        Err(_) => return response::json_err(400, "invalid JSON body"),
    };
    if body.url.is_empty() {
        return response::json_err(400, "url is required");
    }
    match store
        .insert_feed(
            &body.url,
            body.title.as_deref().unwrap_or("Untitled"),
            body.category.as_deref().unwrap_or("uncategorized"),
            body.fetch_interval_sec.unwrap_or(3600),
        )
        .await
    {
        Ok(Some(feed_id)) => {
            // Auto-register a default source entry
            let title = body.title.as_deref().unwrap_or("Untitled");
            let _ = store
                .save_source(&store::NewSource {
                    source_type: "RssFeed".into(),
                    feed_id: Some(feed_id),
                    name: Some(title.into()),
                    tier: "Tier2".into(),
                    policy: "SummaryAllowed".into(),
                    license: "Unknown".into(),
                    license_detail: None,
                    attribution: Some(title.into()),
                    trust_score: None,
                    retention_days: None,
                    verified: false,
                    notes: None,
                })
                .await;
            match store.get_feed(feed_id).await {
                Ok(Some(feed)) => response::json_ok(json!({"feed": feed})),
                _ => response::json_ok(json!({"id": feed_id})),
            }
        }
        Ok(None) => response::json_err(409, "feed with this URL already exists"),
        Err(e) => response::json_err_internal(&e.to_string()),
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

pub(crate) async fn feeds_update(mut req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let store = ctx.data.clone();
    let id = match params::param_i64(&ctx, "id") {
        Some(v) => v,
        None => return response::json_err(400, "invalid id"),
    };
    let body: UpdateFeedBody = match req.json().await {
        Ok(b) => b,
        Err(_) => return response::json_err(400, "invalid JSON body"),
    };
    if let Some(ref status) = body.status {
        if let Err(e) = store.set_feed_status(id, status).await {
            return response::json_err_internal(&e.to_string());
        }
    }
    if let Err(e) = store
        .update_feed(
            id,
            body.title.as_deref(),
            body.category.as_deref(),
            body.fetch_interval_sec,
            body.extraction_level.as_deref(),
        )
        .await
    {
        return response::json_err_internal(&e.to_string());
    }
    match store.get_feed(id).await {
        Ok(Some(feed)) => response::json_ok(json!({"feed": feed})),
        Ok(None) => response::json_err(404, "feed not found"),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

pub(crate) async fn feeds_delete(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let store = ctx.data.clone();
    let id = match params::param_i64(&ctx, "id") {
        Some(v) => v,
        None => return response::json_err(400, "invalid id"),
    };
    match store.set_feed_status(id, "inactive").await {
        Ok(()) => response::json_ok(json!({"status": "deleted", "id": id})),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}
