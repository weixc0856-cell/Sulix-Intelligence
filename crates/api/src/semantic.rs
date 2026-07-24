//! Semantic search endpoint.
//!
//! POST /api/articles/search

use crate::{json_err, json_ok};
use embedding::{build_embedding_text, EmbeddingProvider, WorkersAiEmbedder};
use js_sys::{Object, Reflect};
use serde::Deserialize;
use store::Store;
use vectorize::VectorizeIndex;
use worker::wasm_bindgen::JsCast;
use worker::wasm_bindgen::JsValue;
use worker::*;

#[derive(Deserialize)]
struct SemanticSearchRequest {
    q: String,
    limit: Option<u32>,
}

pub async fn semantic_search(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let body: SemanticSearchRequest = match req.json().await {
        Ok(b) => b,
        Err(_) => return json_err(400, "invalid JSON body"),
    };
    if body.q.trim().is_empty() {
        return json_err(400, "missing query 'q'");
    }

    let store = Store::new(ctx.env.d1("DB")?);
    let vectorize = match ctx.env.get_binding::<VectorizeIndex>("VECTORIZE") {
        Ok(v) => v,
        Err(e) => return json_err(500, &format!("VECTORIZE binding: {e}")),
    };
    let limit = body.limit.unwrap_or(30).min(100);

    // 1. Generate query embedding
    let embedder = WorkersAiEmbedder::new(&ctx.env);
    let embed_text = build_embedding_text(&body.q, "", &[], None);
    let query_emb = match embedder.embed(&embed_text).await {
        Ok(v) => v,
        Err(e) => return json_err(500, &format!("embedding failed: {e}")),
    };

    // 2. Build Vectorize query: JSON vector array + options object
    let vec_str = serde_json::to_string(&query_emb).unwrap_or_else(|_| "[]".to_string());
    let vec_js = match js_sys::JSON::parse(&vec_str) {
        Ok(v) => v,
        Err(e) => {
            console_log!("[Sulix:semantic] query embedding JSON parse failed: {e:?}");
            return json_ok(serde_json::json!({"mode":"semantic","query":body.q,"results":[]}));
        }
    };
    let opts = Object::new();
    let _ = Reflect::set(&opts, &"topK".into(), &JsValue::from_f64(limit as f64));

    let result: JsValue = match vectorize.query(vec_js, opts.into()).await {
        Ok(v) => v,
        Err(e) => return json_err(500, &format!("Vectorize query failed: {e:?}")),
    };

    // 3. Parse matches
    let matches = Reflect::get(&result, &"matches".into())
        .ok()
        .and_then(|v| v.dyn_into::<js_sys::Array>().ok())
        .unwrap_or_default();
    if matches.length() == 0 {
        return json_ok(serde_json::json!({"mode":"semantic","query":body.q,"results":[]}));
    }

    // 4. Extract article IDs + similarity
    let mut article_ids = Vec::new();
    let mut sim_map = std::collections::HashMap::<i64, f64>::new();
    for i in 0..matches.length() {
        let item = matches.get(i);
        let id = Reflect::get(&item, &"id".into()).ok().and_then(|v| v.as_string()).unwrap_or_default();
        let score = Reflect::get(&item, &"score".into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
        if let Some(id_str) = id.strip_prefix("article-") {
            if let Ok(aid) = id_str.parse::<i64>() {
                article_ids.push(aid);
                sim_map.insert(aid, score);
            }
        }
    }

    // 5. Fetch from D1 + enrich
    let articles = match store.articles_by_ids(&article_ids).await {
        Ok(a) => a,
        Err(e) => return json_err(500, &e.to_string()),
    };
    let mut enriched: Vec<serde_json::Value> = articles.into_iter().map(|a| {
        serde_json::json!({"id":a.id,"title":a.title,"url":a.url,"published_at":a.published_at,"ai_summary":a.ai_summary,"ai_tags":a.ai_tags,"similarity":sim_map.get(&a.id).copied().unwrap_or(0.0)})
    }).collect();
    enriched.sort_by(|a, b| {
        b["similarity"]
            .as_f64()
            .unwrap_or(0.0)
            .partial_cmp(&a["similarity"].as_f64().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    json_ok(serde_json::json!({"mode":"semantic","query":body.q,"results":enriched}))
}
