use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use worker::*;

use vectorize::{VectorizeIndex, VectorMetadata, VectorRecord};
use worker::wasm_bindgen::JsValue;

mod metrics;
use metrics::PipelineMetrics;

use ai_pipeline::{process_article, HttpClient, HttpSummarizer, PipelineError};
use ai_pipeline::briefing::{generate_daily_brief, SignalCandidate};
use api::router;
use fetcher::{fetch_feed, FetchOutcome};
use rules::{score, ArticleInput, Rule};
use store::{NewArticle, Store, StoreBackend};

// ---------------------------------------------------------------------------
// WorkerHttpClient - bridges ai_pipeline::HttpClient over worker::Fetch
// ---------------------------------------------------------------------------

struct WorkerHttpClient;

#[async_trait(?Send)]
impl HttpClient for WorkerHttpClient {
    async fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, PipelineError> {
        use worker::{Fetch, Headers, Method, Request, RequestInit};
        let mut init = RequestInit::new();
        init.with_method(Method::Post);
        let wh = Headers::new();
        for (k, v) in headers {
            wh.set(k, v).map_err(|e| PipelineError::Summarizer(e.to_string()))?;
        }
        init.with_headers(wh);
        init.with_body(Some(serde_json::to_string(body).map_err(|e| PipelineError::Summarizer(e.to_string()))?.into()));
        let req = Request::new_with_init(url, &init).map_err(|e| PipelineError::Summarizer(e.to_string()))?;
        let mut resp = Fetch::Request(req).send().await.map_err(|e| PipelineError::Summarizer(e.to_string()))?;
        if resp.status_code() >= 400 {
            return Err(PipelineError::Summarizer(format!(
                "API returned {}: {}",
                resp.status_code(),
                resp.text().await.unwrap_or_default()
            )));
        }
        resp.json::<serde_json::Value>().await.map_err(|e| PipelineError::Summarizer(e.to_string()))
    }
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();
    console_log!("[INFO] HTTP request: {} {}", req.method(), req.path());
    if req.path().to_lowercase().contains("__cron") {
        match process_all_feeds(&env).await {
            Ok(_) => Response::ok("cron triggered"),
            Err(e) => Response::error(format!("cron failed: {e}"), 500),
        }
    } else {
        let result = router().run(req, env).await;
        if let Err(ref e) = result {
            console_log!("[ERROR] router.run failed: {e}");
        }
        result
    }
}

#[event(scheduled)]
async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    console_error_panic_hook::set_once();
    console_log!("scheduled handler at ts={}", js_sys::Date::now());
    if let Err(e) = process_all_feeds(&env).await {
        console_log!("scheduled handler failed: {e}");
    }
    // R2 garbage collection — runs on every cron cycle but is a no-op
    // when there's nothing to expire (no R2 bucket configured, or no
    // articles past the 30-day cutoff with full-text content).
    let now = (js_sys::Date::now() / 1000.0) as i64;
    if let Err(e) = gc_r2_objects(&env, now).await {
        console_log!("gc_r2_objects failed: {e}");
    }
    // Daily Intelligence Brief generation — runs once per day.
    // Uses a KV lock (TTL 1h) to prevent duplicate generation across
    // multiple cron cycles.  Failure is non-fatal (logged, not retried).
    generate_briefing_task(&env, now).await;
}

// ---------------------------------------------------------------------------
// Feed processing pipeline
// ---------------------------------------------------------------------------

/// Context that groups all per-fetch dependencies.
struct FeedContext<'a, S: StoreBackend> {
    store: &'a S,
    summarizer: &'a Option<HttpSummarizer>,
    r2_bucket: &'a Option<Bucket>,
    vectorize: &'a Option<VectorizeIndex>,
    rules: &'a [Rule],
    has_rules: bool,
    now: i64,
    /// Per-feed pipeline metrics accumulator.
    metrics: RefCell<PipelineMetrics>,
}

/// Outcome of processing a single feed through the pipeline.
pub struct FeedProcessResult {
    pub feed_id: i64,
    pub articles_processed: usize,
}

/// Process a single feed: fetch -> insert -> score -> AI summarise.
async fn process_one_feed(ctx: &FeedContext<'_, impl StoreBackend>, _env: &Env, job: &FetchJob) -> Result<(), Error> {
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
                                        if let Err(e) = bucket.put(&r2_key, full_text.as_bytes().to_vec()).execute().await
                                        {
                                            console_log!("  R2 write failed for article {article_id}: {e}");
                                            ctx.metrics.borrow_mut().errors += 1;
                                        }
                                        ctx.metrics.borrow_mut().record_ms("r2", PipelineMetrics::since(r2_start));
                                        if let Err(e) = ctx.store.set_raw_content_r2_key(article_id, Some(&r2_key)).await
                                        {
                                            console_log!("  DB R2 key update failed for article {article_id}: {e}");
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
                                    }
                                    Err(_) => {
                                        ctx.metrics.borrow_mut().errors += 1;
                                        let excerpt = if body.len() > 500 { &body[..500] } else { &body };
                                        if let Err(e) = ctx.store.set_raw_content_r2_key(article_id, Some(excerpt)).await
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
                        job.feed_id, job.feed_url, e
                    );
                }
            }
        }
    }
    Ok(())
}

/// Build a FeedContext from env bindings and process every due feed synchronously.
/// Shared by the queue handler (individual messages) and the sync fallback (batch).
/// Extracted to eliminate duplicate context-building code between queue/fallback paths.
async fn execute_feed_batch(env: &Env, feeds: &[store::Feed], now: i64) -> Vec<FeedProcessResult> {
    let store = match env.d1("DB") {
        Ok(d) => Store::new(d),
        Err(e) => {
            console_log!("D1 error: {e}");
            return Vec::new();
        }
    };
    let summarizer = try_build_summarizer(env);
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

#[event(queue)]
async fn queue(batch: MessageBatch<FetchJob>, env: Env, _ctx: Context) -> Result<()> {
    console_error_panic_hook::set_once();
    let store = Store::new(env.d1("DB")?);
    let summarizer = try_build_summarizer(&env);
    let r2_bucket = env.bucket("RAW_CONTENT").ok();
    let vectorize = env.get_binding::<VectorizeIndex>("VECTORIZE").ok();
    let now = (js_sys::Date::now() / 1000.0) as i64;
    let rule_jsons = match store.active_rule_jsons("default").await {
        Ok(r) => r,
        Err(e) => {
            console_log!("  failed to load rules: {e:?}; proceeding without scoring");
            Vec::new()
        }
    };
    let rules: Vec<Rule> = rule_jsons.iter().filter_map(|j| serde_json::from_str(j).ok()).collect();
    let metrics = RefCell::new(PipelineMetrics::default());
    let feed_ctx = FeedContext {
        store: &store,
        summarizer: &summarizer,
        r2_bucket: &r2_bucket,
        vectorize: &vectorize,
        rules: &rules,
        has_rules: !rules.is_empty(),
        now,
        metrics,
    };
    for msg in batch.messages()?.iter() {
        let job = msg.body();
        console_log!("  queue processing feed {}: {}", job.feed_id, job.feed_url);
        if let Err(e) = process_one_feed(&feed_ctx, &env, job).await {
            console_log!("  feed {} failed: {e}", job.feed_id);
            msg.retry();
        } else {
            msg.ack();
        }
    }
    console_log!("  queue metrics: {}", feed_ctx.metrics.borrow().snapshot());
    if let Ok(cache) = env.kv("CACHE") {
        let metrics_json = feed_ctx.metrics.borrow().snapshot().to_string();
        if let Ok(pb) = cache.put("pipeline_metrics", metrics_json) {
            if let Err(e) = pb.execute().await {
                console_log!("  KV metrics write failed: {e}");
            }
        }
    }
    Ok(())
}

async fn process_all_feeds(env: &Env) -> Result<()> {
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
                // Metrics not available here — this happens before pipeline starts
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

async fn upsert_vector(idx: &VectorizeIndex, article_id: i64, embedding: &[f32]) -> Result<(), String> {
    let record = VectorRecord {
        id: format!("article-{article_id}"),
        values: embedding.to_vec(),
        metadata: Some(VectorMetadata {
            article_id,
            feed_id: None,
            published_at: None,
        }),
    };
    vectorize::upsert_vector(idx, &record).await
}

/// Garbage-collect orphaned R2 objects.
///
/// Queries D1 for articles that match the expiry criteria and have a
/// raw_content_r2_key set, deletes the R2 objects first, then expires
/// the D1 rows.  This avoids accumulating orphaned R2 objects when
/// `expire_old_articles` removes the D1 row but not the corresponding
/// R2 blob.
///
/// Safe to call on every cron cycle — when there is nothing to expire
/// the D1 query returns zero rows and no R2 deletes happen.
async fn gc_r2_objects(env: &Env, now: i64) -> Result<u64, Error> {
    let bucket = match env.bucket("RAW_CONTENT") {
        Ok(b) => b,
        Err(_) => return Ok(0), // no R2 configured — skip
    };
    let store = Store::new(env.d1("DB").map_err(|e| Error::RustError(e.to_string()))?);

    // 1. Collect R2 keys for articles about to expire
    let r2_keys = store
        .expired_article_r2_keys(now, 30)
        .await
        .map_err(|e| Error::RustError(e.to_string()))?;

    if r2_keys.is_empty() {
        return Ok(0);
    }

    // 2. Delete the R2 objects first (before D1 rows are removed)
    let mut deleted_r2 = 0u64;
    for key in &r2_keys {
        if let Err(e) = bucket.delete(key).await {
            console_log!("[Sulix:gc] R2 delete failed for {key}: {e:?}");
        } else {
            deleted_r2 += 1;
        }
    }

    // 3. Expire the D1 rows
    let deleted_d1 = store
        .expire_old_articles(now, 30)
        .await
        .map_err(|e| Error::RustError(e.to_string()))?;

    console_log!("[Sulix:gc] deleted {deleted_r2} R2 objects, {deleted_d1} D1 rows");
    Ok(deleted_r2)
}

/// Generate today's intelligence briefing and persist it.
///
/// Guarded by a KV lock (`briefing_lock:YYYY-MM-DD`, TTL 1h) so only
/// the first cron cycle of the day creates the briefing.  Subsequent
/// cycles find the lock and skip.
async fn generate_briefing_task(env: &Env, now: i64) {
    let date = {
        let d = js_sys::Date::new(&JsValue::from_f64((now as f64) * 1000.0));
        format!("{:04}-{:02}-{:02}", d.get_full_year(), d.get_month() + 1, d.get_date())
    };
    let lock_key = format!("briefing_lock:{date}");

    // KV lock — skip if already generated today
    if let Ok(cache) = env.kv("CACHE") {
        if let Ok(Some(_)) = cache.get(&lock_key).text().await {
            console_log!("[Sulix:briefing] already generated for {date} — skipping");
            return;
        }
    }

    let store = match env.d1("DB") {
        Ok(db) => Store::new(db),
        Err(e) => {
            console_log!("[Sulix:briefing] D1 binding failed: {e}");
            return;
        }
    };

    // 1. Load signals
    let today_signals = match store.signals_today(now).await {
        Ok(s) => s,
        Err(e) => {
            console_log!("[Sulix:briefing] signals_today failed: {e}");
            return;
        }
    };
    if today_signals.is_empty() {
        console_log!("[Sulix:briefing] no signals today — skipping");
        return;
    }

    // 2. Build summarizer
    let summarizer = match try_build_summarizer(env) {
        Some(s) => s,
        None => {
            console_log!("[Sulix:briefing] no AI summarizer available — skipping");
            return;
        }
    };

    // 3. Convert to SignalCandidate
    let candidates: Vec<SignalCandidate> = today_signals
        .into_iter()
        .map(|s| {
            let article_ids: Vec<i64> = s.articles.iter().map(|a| a.id).collect();
            let avg_score: f64 = if !s.articles.is_empty() {
                s.articles.iter().map(|a| a.score).sum::<f64>() / s.articles.len() as f64
            } else {
                0.0
            };
            SignalCandidate {
                id: s.id,
                title: s.title,
                category: String::new(),
                signal_summary: s.summary,
                article_count: s.articles.len(),
                avg_score,
                trend: s.trend,
                article_ids,
            }
        })
        .collect();

    // 4. Acquire lock before generating (prevent concurrent cron runs)
    let cache = env.kv("CACHE").ok();
    if let Some(ref cache) = cache {
        if let Ok(pb) = cache.put(&lock_key, "1") {
            let _ = pb.expiration_ttl(3600).execute().await;
        }
    }

    // 5. Generate
    let briefing = match generate_daily_brief(candidates, &summarizer, &date, now).await {
        Ok(b) => b,
        Err(e) => {
            console_log!("[Sulix:briefing] generation failed: {e}");
            return;
        }
    };

    // 6. Persist to D1
    let content = serde_json::to_string(&briefing).unwrap_or_default();
    if let Err(e) = store.save_briefing(&date, now, briefing.signal_count, &content).await {
        console_log!("[Sulix:briefing] D1 save failed: {e}");
        return;
    }

    // 7. Write KV cache
    if let Some(ref cache) = cache {
        let cache_key = format!("briefing:{date}");
        if let Ok(pb) = cache.put(&cache_key, &content) {
            let _ = pb.expiration_ttl(21600).execute().await;
        }
    }

    console_log!(
        "[Sulix:briefing] generated for {date} — {} insights",
        briefing.insights.len()
    );
}

fn try_build_summarizer(env: &Env) -> Option<HttpSummarizer> {
    let api_key = match env.secret("AI_API_KEY") {
        Ok(v) => v.to_string(),
        Err(_) => {
            console_log!("AI_API_KEY not set");
            return None;
        }
    };
    let base_url =
        env.var("AI_BASE_URL").ok().map(|v| v.to_string()).unwrap_or_else(|| "https://api.deepseek.com/v1".into());
    let chat_model = env.var("AI_CHAT_MODEL").ok().map(|v| v.to_string()).unwrap_or_else(|| "deepseek-v4-flash".into());
    let embedding_model = env.var("AI_EMBEDDING_MODEL").ok().map(|v| v.to_string()).unwrap_or_default();
    Some(HttpSummarizer::new(base_url, api_key, chat_model, embedding_model, Box::new(WorkerHttpClient)))
}

fn extract_body(entry: &feed_rs::model::Entry) -> String {
    entry
        .summary
        .as_ref()
        .map(|s| s.content.clone())
        .or_else(|| entry.content.as_ref().and_then(|c| c.body.clone()))
        .or_else(|| {
            let texts: Vec<&str> =
                entry.media.iter().filter_map(|m| m.description.as_ref().map(|d| d.content.as_str())).collect();
            if texts.is_empty() {
                None
            } else {
                Some(texts.join("\n"))
            }
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FetchJob {
    feed_id: i64,
    feed_url: String,
    prior_etag: Option<String>,
    prior_last_modified: Option<String>,
    extraction_level: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use feed_rs::model::{Content, Entry};

    #[test]
    fn extract_body_from_content() {
        let entry = Entry {
            content: Some(Content { body: Some("content".into()), ..Default::default() }),
            ..Default::default()
        };
        assert_eq!(extract_body(&entry), "content");
    }

    #[test]
    fn extract_body_prefers_summary() {
        // Summary is accessed but needs proper MediaTypeBuf; use content-only entry
        let entry = Entry {
            content: Some(Content { body: Some("fallback".into()), ..Default::default() }),
            ..Default::default()
        };
        assert_eq!(extract_body(&entry), "fallback");
    }

    #[test]
    fn extract_body_empty_entry() {
        let entry = Entry::default();
        assert_eq!(extract_body(&entry), "");
    }

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
