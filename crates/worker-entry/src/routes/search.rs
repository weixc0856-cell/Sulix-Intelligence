//! FTS article search endpoint — composition-root owned.
//!
//! GET /api/articles/search?q=...
//!
//! Migrated from `api` in Phase 2: `search_articles` talks to D1 FTS5 through
//! `D1FtsSearch` directly against the `DB` binding — an infrastructure-facing
//! HTTP endpoint that lives in worker-entry. Wiring only.

use serde_json::json;
use store::Store;
use worker::*;

use search::D1FtsSearch;

use super::response;

/// GET /api/articles/search — full-text search via D1 FTS5.
pub(crate) async fn search_articles(req: Request, ctx: RouteContext<Store>) -> Result<Response> {
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
    let limit = parse_limit(&url);
    let offset = parse_offset(&url);

    let total = search.search_count(&query, tag.as_deref(), category.as_deref()).await.unwrap_or(0);

    match search.search_filtered(&query, limit, offset, tag.as_deref(), category.as_deref(), sort.as_deref()).await {
        Ok(hits) => response::json_ok(json!({"results": hits, "total": total, "limit": limit, "offset": offset})),
        Err(e) => response::json_err_internal(&e.to_string()),
    }
}

// Duplicated from api's `shared::params` so this self-contained route file does
// not reach into the api crate's private helpers.

fn parse_limit(url: &Url) -> u32 {
    url.query_pairs().find(|(k, _)| k == "limit").and_then(|(_, v)| v.parse().ok()).unwrap_or(30)
}

fn parse_offset(url: &Url) -> u32 {
    url.query_pairs().find(|(k, _)| k == "offset").and_then(|(_, v)| v.parse().ok()).unwrap_or(0)
}
