use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use worker::*;

use crate::extract_body;
use crate::jobs::embedding::upsert_vector;
use crate::metrics::PipelineMetrics;
use crate::version::PIPELINE_VERSION;
use ai_pipeline::{process_article, HttpSummarizer};
use entity::{canonicalizer, classifier};
use fetcher::{fetch_feed, FetchOutcome};
use rules::{score, ArticleInput, Rule};
use store::{NewArticle, NewArtifact, Store, StoreBackend};
use vectorize::VectorizeIndex;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Context that groups all per-fetch dependencies.
pub(crate) struct FeedContext<'a, S: StoreBackend> {
    pub(crate) store: &'a S,
    pub(crate) summarizer: &'a Option<HttpSummarizer>,
    pub(crate) r2_bucket: &'a Option<Bucket>,
    pub(crate) vectorize: &'a Option<VectorizeIndex>,
    pub(crate) rules: &'a [Rule],
    pub(crate) has_rules: bool,
    pub(crate) now: i64,
    pub(crate) metrics: RefCell<PipelineMetrics>,
}

/// Outcome of processing a single feed through the pipeline.
#[allow(dead_code)]
pub(crate) struct FeedProcessResult {
    pub(crate) feed_id: i64,
    pub(crate) articles_processed: usize,
}

/// Queue message describing a single feed to fetch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FetchJob {
    pub(crate) feed_id: i64,
    pub(crate) feed_url: String,
    pub(crate) prior_etag: Option<String>,
    pub(crate) prior_last_modified: Option<String>,
    pub(crate) extraction_level: String,
}

// ---------------------------------------------------------------------------
// Feed processing pipeline
// ---------------------------------------------------------------------------

/// Process a single feed: fetch -> insert -> score -> AI summarise.
pub(crate) async fn process_one_feed(
    ctx: &FeedContext<'_, impl StoreBackend>,
    _env: &Env,
    job: &FetchJob,
) -> Result<(), Error> {
    let do_ai = ctx.summarizer.is_some();
    match fetch_feed(&job.feed_url, job.prior_etag.as_deref(), job.prior_last_modified.as_deref()).await {
        Ok(FetchOutcome::NotModified) => {
            let start = js_sys::Date::now();
            if let Err(e) = ctx.store.record_fetch_result(job.feed_id, ctx.now, None, None).await {
                console_log!("    record_fetch_result failed: {e}");
            }
            ctx.metrics.borrow_mut().record_ms("store", PipelineMetrics::since(start));
            ctx.metrics.borrow_mut().articles_fetched += 1;
        }
        Ok(FetchOutcome::Updated(fetched)) => {
            ctx.metrics.borrow_mut().articles_fetched += 1;
            for entry in fetched.feed.entries {
                let feed_summary = extract_body(&entry);
                let mut body = feed_summary.clone();
                let article = NewArticle {
                    feed_id: job.feed_id,
                    guid: entry.id.clone(),
                    title: entry.title.map(|t| t.content).unwrap_or_default(),
                    url: entry.links.first().map(|l| l.href.clone()),
                    published_at: entry.published.map(|d| d.timestamp()).filter(|&ts| ts <= ctx.now),
                    raw_content_r2_key: None,
                };
                let start = js_sys::Date::now();
                match ctx.store.insert_article(&article).await {
                    Ok(Some(article_id)) => {
                        ctx.metrics.borrow_mut().record_ms("store", PipelineMetrics::since(start));
                        ctx.metrics.borrow_mut().articles_new += 1;
                        let article_score = if ctx.has_rules {
                            score(
                                &ArticleInput { title: &article.title, summary: &body, feed_url: &job.feed_url },
                                ctx.rules,
                                "default",
                            )
                        } else {
                            0.0
                        };
                        if job.extraction_level == "full_text" {
                            if let Some(ref url) = article.url {
                                let fr_start = js_sys::Date::now();
                                if let Ok(full_text) = fetcher::extract_full_text(url).await {
                                    ctx.metrics.borrow_mut().record_ms("fetch", PipelineMetrics::since(fr_start));
                                    let r2_key = format!("articles/{article_id}");
                                    if let Some(ref bucket) = ctx.r2_bucket {
                                        let r2_start = js_sys::Date::now();
                                        if let Err(e) =
                                            bucket.put(&r2_key, full_text.as_bytes().to_vec()).execute().await
                                        {
                                            console_log!("  R2 write failed for article {article_id}: {e}");
                                            ctx.metrics.borrow_mut().errors += 1;
                                        }
                                        ctx.metrics.borrow_mut().record_ms("r2", PipelineMetrics::since(r2_start));
                                        if let Err(e) =
                                            ctx.store.set_raw_content_r2_key(article_id, Some(&r2_key)).await
                                        {
                                            console_log!("  DB R2 key update failed for article {article_id}: {e}");
                                        }
                                        // Register artifact in artifact_registry for traceability
                                        if let Err(e) = ctx
                                            .store
                                            .create_artifact(&NewArtifact {
                                                artifact_type: "article_snapshot".into(),
                                                entity_id: article_id,
                                                r2_key: r2_key.clone(),
                                                schema_version: "article.v1".into(),
                                                model: None,
                                                pipeline_version: PIPELINE_VERSION.into(),
                                                metadata: None,
                                            })
                                            .await
                                        {
                                            console_log!(
                                                "  artifact registry write failed for article {article_id}: {e}"
                                            );
                                        }
                                    }
                                    body = full_text;
                                }
                            }
                        }
                        if do_ai {
                            if let Some(ref s) = ctx.summarizer {
                                let llm_start = js_sys::Date::now();
                                match process_article(ctx.store, s, article_id, &article.title, &body, article_score)
                                    .await
                                {
                                    Ok(result) => {
                                        ctx.metrics.borrow_mut().record_ms("llm", PipelineMetrics::since(llm_start));
                                        if !result.embedding.is_empty() {
                                            if let Some(ref idx) = ctx.vectorize {
                                                let emb_start = js_sys::Date::now();
                                                if let Err(e) = upsert_vector(idx, article_id, &result.embedding).await
                                                {
                                                    console_log!(
                                                        "  vectorize upsert failed for article {article_id}: {e}"
                                                    );
                                                    ctx.metrics.borrow_mut().errors += 1;
                                                }
                                                ctx.metrics
                                                    .borrow_mut()
                                                    .record_ms("embedding", PipelineMetrics::since(emb_start));
                                            }
                                        }
                                        // Entity persistence — extract, classify, and link named entities
                                        if !result.entities.is_empty() {
                                            let mut entity_ids: Vec<i64> = Vec::new();
                                            for entity_name in &result.entities {
                                                let normalized = canonicalizer::normalize(entity_name);
                                                let entity_type = classifier::classify(entity_name);
                                                match ctx
                                                    .store
                                                    .upsert_entity(entity_name, &normalized, entity_type)
                                                    .await
                                                {
                                                    Ok(eid) => {
                                                        ctx.metrics.borrow_mut().entities_created += 1;
                                                        entity_ids.push(eid);
                                                        if let Err(e) = ctx
                                                            .store
                                                            .link_article_entity(article_id, eid, 1.0, None)
                                                            .await
                                                        {
                                                            console_log!(
                                                                "  entity link failed for article {article_id}: {e}"
                                                            );
                                                        } else {
                                                            ctx.metrics.borrow_mut().entity_links += 1;
                                                        }
                                                    }
                                                    Err(e) => {
                                                        console_log!("  entity upsert failed for '{entity_name}': {e}");
                                                    }
                                                }
                                            }
                                            // Build mentioned_together co-occurrence relations
                                            // Sprint 5.10: cap at Top-5 entities by score (max 10 relations/article)
                                            let top_n = entity_ids.len().min(5);
                                            for i in 0..top_n {
                                                for j in (i + 1)..top_n {
                                                    if let Err(e) = ctx
                                                        .store
                                                        .link_entity_relation(
                                                            entity_ids[i],
                                                            entity_ids[j],
                                                            "mentioned_together",
                                                            1.0,
                                                        )
                                                        .await
                                                    {
                                                        console_log!("  entity relation failed: {e}");
                                                    } else {
                                                        ctx.metrics.borrow_mut().entity_relations += 1;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        ctx.metrics.borrow_mut().errors += 1;
                                        let excerpt = crate::utils::truncate_chars(&body, 500);
                                        if let Err(e) =
                                            ctx.store.set_raw_content_r2_key(article_id, Some(excerpt)).await
                                        {
                                            console_log!(
                                                "  DB excerpt write failed for article {article_id} (LLM already failed): {e}"
                                            );
                                        }
                                    }
                                }
                            }
                        } else if article_score != 0.0 {
                            if let Err(e) = ctx
                                .store
                                .set_ai_summary(article_id, "", "[]", &format!("article-{article_id}"), article_score)
                                .await
                            {
                                console_log!("  DB score update failed for article {article_id}: {e}");
                            }
                        }
                    }
                    Ok(None) => {
                        ctx.metrics.borrow_mut().articles_dup += 1;
                    }
                    Err(e) => {
                        console_log!("    insert_article failed: {e}");
                        ctx.metrics.borrow_mut().errors += 1;
                    }
                }
            }
            let start = js_sys::Date::now();
            if let Err(e) = ctx
                .store
                .record_fetch_result(job.feed_id, ctx.now, fetched.etag.as_deref(), fetched.last_modified.as_deref())
                .await
            {
                console_log!("  failed to persist fetch result for feed {} (url={}): {e}", job.feed_id, job.feed_url);
            }
            ctx.metrics.borrow_mut().record_ms("store", PipelineMetrics::since(start));
        }
        Err(e) => {
            console_log!("    fetch_feed failed: {e}");
            ctx.metrics.borrow_mut().errors += 1;
            if !e.is_transient() {
                if let Err(db_err) = ctx.store.record_fetch_result(job.feed_id, ctx.now, None, None).await {
                    console_log!(
                        "  failed to record fetch error for feed {} (url={}, fetch_err={}): {db_err}",
                        job.feed_id,
                        job.feed_url,
                        e
                    );
                }
            }
        }
    }
    Ok(())
}

/// Build a FeedContext from env bindings and process every due feed synchronously.
pub(crate) async fn execute_feed_batch(env: &Env, feeds: &[store::Feed], now: i64) -> Vec<FeedProcessResult> {
    let store = match env.d1("DB") {
        Ok(d) => Store::new(d),
        Err(e) => {
            console_log!("D1 error: {e}");
            return Vec::new();
        }
    };
    let summarizer = crate::services::summarizer::try_build_summarizer(env);
    let r2_bucket = env.bucket("RAW_CONTENT").ok();
    let vectorize = env.get_binding::<VectorizeIndex>("VECTORIZE").ok();
    let rule_jsons = match store.active_rule_jsons("default").await {
        Ok(r) => r,
        Err(e) => {
            console_log!("  failed to load rules: {e:?}; proceeding without scoring");
            Vec::new()
        }
    };
    let rules: Vec<Rule> = rule_jsons.iter().filter_map(|j| serde_json::from_str(j).ok()).collect();
    let metrics = RefCell::new(PipelineMetrics::default());
    let ctx = FeedContext {
        store: &store,
        summarizer: &summarizer,
        r2_bucket: &r2_bucket,
        vectorize: &vectorize,
        rules: &rules,
        has_rules: !rules.is_empty(),
        now,
        metrics,
    };

    let mut results = Vec::with_capacity(feeds.len());
    for feed in feeds {
        let job = FetchJob {
            feed_id: feed.id,
            feed_url: feed.url.clone(),
            prior_etag: feed.etag.clone(),
            prior_last_modified: feed.last_modified.clone(),
            extraction_level: feed.extraction_level.clone(),
        };
        match process_one_feed(&ctx, env, &job).await {
            Ok(()) => results.push(FeedProcessResult { feed_id: feed.id, articles_processed: 0 }),
            Err(e) => console_log!("  feed {} pipeline error: {e}", feed.id),
        }
    }
    // Persist metrics to KV so the API can serve them
    if let Ok(cache) = env.kv("CACHE") {
        let metrics_json = ctx.metrics.borrow().snapshot().to_string();
        if let Ok(pb) = cache.put("pipeline_metrics", metrics_json) {
            if let Err(e) = pb.execute().await {
                console_log!("  KV metrics write failed: {e}");
            }
        }
    }
    console_log!("  metrics: {}", ctx.metrics.borrow().snapshot());
    results
}

/// Enqueue all due feeds for the pipeline, or process them inline if no queue is bound.
pub(crate) async fn process_all_feeds(env: &Env) -> Result<()> {
    let store = Store::new(env.d1("DB")?);
    let now = (js_sys::Date::now() / 1000.0) as i64;
    let feeds = store.feeds_due_for_fetch(now, None).await.map_err(|e| Error::RustError(e.to_string()))?;
    console_log!("process_all_feeds: {} feeds due, sending to queue", feeds.len());

    let queue = env.queue("FETCH_QUEUE").ok();
    if let Some(ref q) = queue {
        for feed in &feeds {
            let job = FetchJob {
                feed_id: feed.id,
                feed_url: feed.url.clone(),
                prior_etag: feed.etag.clone(),
                prior_last_modified: feed.last_modified.clone(),
                extraction_level: feed.extraction_level.clone(),
            };
            if let Err(e) = q.send(job).await {
                console_log!("  failed to enqueue feed {}: {e}", feed.id);
            }
        }
    } else {
        console_log!("  FETCH_QUEUE not bound, processing via execute_feed_batch");
        execute_feed_batch(env, &feeds, now).await;
    }

    if let Err(e) = store.expire_old_articles(now, 30).await {
        console_log!("expire_old_articles failed: {e}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_job_roundtrip() {
        let job = FetchJob {
            feed_id: 42,
            feed_url: "https://example.com/feed".into(),
            prior_etag: Some("abc".into()),
            prior_last_modified: None,
            extraction_level: "full_text".into(),
        };
        let json = serde_json::to_string(&job).unwrap();
        let de: FetchJob = serde_json::from_str(&json).unwrap();
        assert_eq!(de.feed_id, 42);
        assert_eq!(de.feed_url, "https://example.com/feed");
        assert_eq!(de.prior_etag, Some("abc".into()));
    }

    #[test]
    fn feed_process_result_construction() {
        let r = FeedProcessResult { feed_id: 1, articles_processed: 0 };
        assert_eq!(r.feed_id, 1);
    }
}
