//! Admin endpoint to trigger bulk embedding rebuild.
//!
//! POST /api/admin/rebuild-embeddings
//!
//! Scans articles pending AI processing, generates embeddings via
//! Workers AI, and upserts to Vectorize. Limited to 50 articles per
//! call to stay within Workers CPU time limits.

use crate::{json_err_internal, json_ok};
use embedding::{build_embedding_text, EmbeddingProvider, WorkersAiEmbedder};
use store::Store;
use vectorize::{VectorizeIndex, VectorMetadata, VectorRecord};
use worker::*;

pub async fn rebuild_embeddings(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let vectorize = match ctx.env.get_binding::<VectorizeIndex>("VECTORIZE") {
        Ok(v) => v,
        Err(e) => return json_err_internal(&format!("VECTORIZE binding: {e}")),
    };
    let embedder = WorkersAiEmbedder::new(&ctx.env);

    let articles = match store.pending_ai_articles(50).await {
        Ok(a) => a,
        Err(e) => return json_err_internal(&e.to_string()),
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
        let tags: Vec<String> =
            article.ai_tags.as_deref().and_then(|t| serde_json::from_str(t).ok()).unwrap_or_default();
        let embed_text = build_embedding_text(&article.title, &article.ai_summary, &tags, None);

        match embedder.embed(&embed_text).await {
            Ok(embedding) => {
                let record = VectorRecord {
                    id: format!("article-{}", article.id),
                    values: embedding.clone(),
                    metadata: Some(VectorMetadata {
                        article_id: article.id,
                        feed_id: Some(article.feed_id),
                        published_at: article.published_at,
                    }),
                };
                match vectorize::upsert_vector(&vectorize, &record).await {
                    Ok(_) => processed += 1,
                    Err(e) => {
                        console_log!("[Sulix:rebuild] upsert failed for article {}: {e:?}", article.id);
                        errors += 1;
                    }
                }
            }
            Err(e) => {
                console_log!("[Sulix:rebuild] embedding failed for article {}: {e}", article.id);
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
