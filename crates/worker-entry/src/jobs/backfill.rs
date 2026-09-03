//! Backfill — process backlog of articles missing AI summaries.

use ai_pipeline::process_article;
use entity::{canonicalizer, classifier};
use infrastructure::article_persistence::D1ArticlePersistence;
use store::D1Store;
use vectorize::VectorizeIndex;
use worker::*;

use crate::jobs::embedding::embed_and_upsert;
use crate::services::embedder::try_build_embedder;
use crate::services::summarizer::try_build_summarizer;

const MAX_PER_CYCLE: u32 = 50;

pub(crate) async fn process_backfill(env: &Env, _now: i64) {
    let store = match env.d1("DB") {
        Ok(db) => D1Store::new(db),
        Err(e) => {
            console_log!("[backfill] D1 binding failed: {e}");
            return;
        }
    };

    let summarizer = try_build_summarizer(env);
    if summarizer.is_none() {
        console_log!("[backfill] summarizer unavailable — skipping");
        return;
    }
    let embedder = try_build_embedder(env);

    let r2_bucket = env.bucket("RAW_CONTENT").ok();
    let vectorize: Option<VectorizeIndex> = env.get_binding("VECTORIZE").ok();

    let rows = match store.get_backfill_candidates(0, MAX_PER_CYCLE).await {
        Ok(r) => r,
        Err(e) => {
            console_log!("[backfill] query failed: {e}");
            return;
        }
    };

    if rows.is_empty() {
        console_log!("[backfill] no backlog — all caught up!");
        return;
    }

    console_log!("[backfill] processing {} articles", rows.len());
    let mut processed = 0u32;

    for article in &rows {
        let article_id = article.id;

        if article.vector_id.as_ref().is_some_and(|v| !v.is_empty()) {
            continue;
        }

        // Read body from R2 if available
        let body = match article.raw_content_r2_key.as_deref() {
            Some(key) if !key.is_empty() => {
                if let Some(ref bucket) = r2_bucket {
                    match bucket.get(key).execute().await {
                        Ok(Some(obj)) => match obj.body() {
                            Some(body_content) => body_content.bytes().await.unwrap_or_default(),
                            None => Vec::new(),
                        },
                        _ => Vec::new(),
                    }
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        };
        let body_str: String = String::from_utf8_lossy(&body).into();

        if let Some(ref s) = summarizer {
            let persistence = D1ArticlePersistence::new(&store);
            match process_article(&persistence, s, article_id, &article.title, &body_str, article.score).await {
                Ok(result) => {
                    // Embedding is a separate step — summary was already persisted by
                    // process_article, so failure here is logged and leaves the article
                    // re-embeddable via POST /api/admin/rebuild-embeddings.
                    if let (Some(e), Some(idx)) = (embedder.as_ref(), vectorize.as_ref()) {
                        if let Err(err) =
                            embed_and_upsert(e, idx, article_id, &article.title, &result.summary, &result.tags).await
                        {
                            console_log!("[backfill] embed/vectorize failed for article {article_id}: {err}");
                        }
                    }
                    if !result.entities.is_empty() {
                        let mut entity_ids: Vec<i64> = Vec::new();
                        for entity_name in &result.entities {
                            let normalized = canonicalizer::normalize(entity_name);
                            let entity_type = classifier::classify(entity_name);
                            if let Ok(eid) = store.upsert_entity(entity_name, &normalized, entity_type).await {
                                entity_ids.push(eid);
                                let _ = store.link_article_entity(article_id, eid, 1.0, None).await;
                            }
                        }
                        for i in 0..entity_ids.len().min(5) {
                            for j in (i + 1)..entity_ids.len().min(5) {
                                let _ = store
                                    .link_entity_relation(entity_ids[i], entity_ids[j], "mentioned_together", 1.0)
                                    .await;
                            }
                        }
                    }
                    processed += 1;
                }
                Err(e) => {
                    console_log!("[backfill] article {article_id} failed: {e}");
                }
            }
        }
    }

    console_log!("[backfill] cycle complete: {}/{} processed", processed, rows.len());
}
