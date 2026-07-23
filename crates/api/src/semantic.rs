//! Semantic search endpoint.
//!
//! POST /api/articles/search
//!
//! Accepts a JSON body with `q`, `mode: "semantic"`, and optional `limit`.
//! Generates a query embedding via Workers AI, searches Vectorize for
//! nearest neighbors, and enriches results with article details from D1.

use crate::{json_err, json_ok};
use embedding::{build_embedding_text, EmbeddingProvider, WorkersAiEmbedder};
use serde::Deserialize;
use store::Store;
use worker::*;
use worker::wasm_bindgen::prelude::*;
use worker::EnvBinding;

use js_sys::{Array, Float32Array, Object, Reflect};

// ---- Vectorize binding (inlined — api doesn't depend on worker-entry) ----

#[wasm_bindgen]
extern "C" {
    pub type VectorizeIndex;

    #[wasm_bindgen(method, catch)]
    async fn query(this: &VectorizeIndex, vector: JsValue, opts: JsValue) -> Result<JsValue, JsValue>;
}

impl EnvBinding for VectorizeIndex {
    const TYPE_NAME: &'static str = "VectorizeIndex";
}

#[derive(Deserialize)]
struct SemanticSearchRequest {
    q: String,
    #[allow(dead_code)]
    mode: Option<String>,
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

    // 1. Generate query embedding via Workers AI
    let embedder = WorkersAiEmbedder::new(&ctx.env);
    let embed_text = build_embedding_text(&body.q, "", &[], None);
    let query_emb = match embedder.embed(&embed_text).await {
        Ok(v) => v,
        Err(e) => return json_err(500, &format!("embedding failed: {e}")),
    };

    // 2. Build Vectorize query
    let values = Float32Array::new_with_length(query_emb.len() as u32);
    for (i, v) in query_emb.iter().enumerate() {
        values.set_index(i as u32, *v);
    }
    let vector_obj = Object::new();
    let _ = Reflect::set(&vector_obj, &"vector".into(), &values.into());

    let opts = Object::new();
    let _ = Reflect::set(&opts, &"topK".into(), &JsValue::from_f64(limit as f64));
    let _ = Reflect::set(&opts, &"returnMetadata".into(), &JsValue::from_bool(false));

    let result: JsValue = match vectorize.query(vector_obj.into(), opts.into()).await {
        Ok(v) => v,
        Err(e) => return json_err(500, &format!("Vectorize query failed: {e:?}")),
    };

    // 3. Parse matches
    let matches = Reflect::get(&result, &"matches".into())
        .ok()
        .and_then(|v| v.dyn_into::<Array>().ok())
        .unwrap_or_else(Array::new);

    if matches.length() == 0 {
        return json_ok(serde_json::json!({
            "mode": "semantic", "query": body.q, "results": []
        }));
    }

    // 4. Extract article IDs and build similarity map
    let mut article_ids = Vec::new();
    let mut sim_map = std::collections::HashMap::<i64, f64>::new();

    for i in 0..matches.length() {
        let item = matches.get(i);
        let id = Reflect::get(&item, &"id".into())
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        let score = Reflect::get(&item, &"score".into())
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        if let Some(id_str) = id.strip_prefix("article-") {
            if let Ok(aid) = id_str.parse::<i64>() {
                article_ids.push(aid);
                sim_map.insert(aid, score);
            }
        }
    }

    // 5. Fetch articles from D1
    let articles = match store.articles_by_ids(&article_ids).await {
        Ok(a) => a,
        Err(e) => return json_err(500, &e.to_string()),
    };

    // 6. Enrich with similarity
    let mut enriched: Vec<serde_json::Value> = articles
        .into_iter()
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "title": a.title,
                "url": a.url,
                "published_at": a.published_at,
                "ai_summary": a.ai_summary,
                "ai_tags": a.ai_tags,
                "similarity": sim_map.get(&a.id).copied().unwrap_or(0.0),
            })
        })
        .collect();

    // Sort by similarity descending (Vectorize order)
    enriched.sort_by(|a, b| {
        b["similarity"]
            .as_f64()
            .unwrap_or(0.0)
            .partial_cmp(&a["similarity"].as_f64().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    json_ok(serde_json::json!({
        "mode": "semantic",
        "query": body.q,
        "results": enriched,
    }))
}
