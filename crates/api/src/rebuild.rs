//! Admin endpoint to trigger bulk embedding rebuild.
//!
//! POST /api/admin/rebuild-embeddings
//!
//! Scans articles pending AI processing, generates embeddings via
//! Workers AI, and upserts to Vectorize. Limited to 50 articles per
//! call to stay within Workers CPU time limits.

use crate::{json_err, json_ok};
use embedding::{build_embedding_text, EmbeddingProvider, WorkersAiEmbedder};
use store::Store;
use worker::*;
use worker::wasm_bindgen::prelude::*;
use worker::EnvBinding;

use js_sys::{Array, Object, Reflect};

#[wasm_bindgen]
extern "C" {
    pub type VectorizeIndex;

    #[wasm_bindgen(method, catch)]
    async fn upsert(this: &VectorizeIndex, vectors: JsValue) -> Result<JsValue, JsValue>;
}

impl EnvBinding for VectorizeIndex {
    const TYPE_NAME: &'static str = "VectorizeIndexImpl";
}

pub async fn rebuild_embeddings(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let vectorize = match ctx.env.get_binding::<VectorizeIndex>("VECTORIZE") {
        Ok(v) => v,
        Err(e) => return json_err(500, &format!("VECTORIZE binding: {e}")),
    };
    let embedder = WorkersAiEmbedder::new(&ctx.env);

    let articles = match store.pending_ai_articles(50).await {
        Ok(a) => a,
        Err(e) => return json_err(500, &e.to_string()),
    };

    if articles.is_empty() {
        return json_ok(serde_json::json!({
            "processed": 0,
            "message": "All articles already have embeddings"
        }));
    }

    let mut processed = 0u64;
    let mut errors = 0u64;

    for article in &articles {
        let tags: Vec<String> = article.ai_tags.as_deref().and_then(|t| serde_json::from_str(t).ok()).unwrap_or_default();
        let embed_text = build_embedding_text(&article.title, &article.ai_summary, &tags, None);

        match embedder.embed(&embed_text).await {
            Ok(embedding) => {
                // Build Vectorize upsert JS object
                let vec_obj = Object::new();
                let _ = Reflect::set(&vec_obj, &"id".into(), &format!("article-{}", article.id).into());
                let vals_str = serde_json::to_string(&embedding).unwrap_or_else(|_| "[]".to_string());
                let vals_js = js_sys::JSON::parse(&vals_str).unwrap_or(JsValue::NULL);
                let _ = Reflect::set(&vec_obj, &"values".into(), &vals_js);

                let meta_obj = Object::new();
                let _ = Reflect::set(&meta_obj, &"article_id".into(), &JsValue::from_f64(article.id as f64));
                let _ = Reflect::set(&meta_obj, &"feed_id".into(), &JsValue::from_f64(article.feed_id as f64));
                let _ = Reflect::set(&meta_obj, &"embedding_model".into(), &"bge-large-en-v1.5".into());
                let _ = Reflect::set(&meta_obj, &"embedding_version".into(), &JsValue::from_f64(1.0));
                let _ = Reflect::set(&meta_obj, &"language".into(), &"en".into());
                let _ = Reflect::set(&meta_obj, &"published_at".into(), &JsValue::from_f64(article.published_at.unwrap_or(0) as f64));
                let _ = Reflect::set(&vec_obj, &"metadata".into(), &meta_obj.into());

                let vectors = Array::new();
                vectors.push(&vec_obj);

                match vectorize.upsert(vectors.into()).await {
                    Ok(_) => processed += 1,
                    Err(e) => {
                        console_log!("  upsert failed for article {}: {e:?}", article.id);
                        errors += 1;
                    }
                }
            }
            Err(e) => {
                console_log!("  embedding failed for article {}: {e}", article.id);
                errors += 1;
            }
        }
    }

    json_ok(serde_json::json!({
        "processed": processed,
        "errors": errors,
        "remaining": articles.len().saturating_sub(processed as usize + errors as usize),
    }))
}
