use serde::{Deserialize, Serialize};
use worker::*;

use ai_pipeline::{process_article, HttpSummarizer};
use api::router;
use fetcher::{fetch_feed, FetchOutcome};
use rules::{score, ArticleInput, Rule};
use store::{NewArticle, Store};

/// HTTP entry point.  `/__cron` triggers the fetch pipeline for debugging.
#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();
    if req.path().to_lowercase().contains("__cron") {
        console_log!("manual cron trigger via HTTP");
        match process_all_feeds(&env).await {
            Ok(_) => Response::ok("cron triggered"),
            Err(e) => Response::error(format!("cron failed: {e}"), 500),
        }
    } else {
        router().run(req, env).await
    }
}

/// Cron handler: iterate due feeds, distribute to queue for async processing.
#[event(scheduled)]
async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    console_error_panic_hook::set_once();
    let ts = js_sys::Date::now();
    console_log!("scheduled handler at ts={ts}");
    if let Err(e) = process_all_feeds(&env).await {
        console_log!("scheduled handler failed: {e}");
    }
    console_log!("scheduled handler completed");
}

/// Queue consumer: process one fetch job per message.
#[event(queue)]
async fn queue(batch: MessageBatch<FetchJob>, env: Env, _ctx: Context) -> Result<()> {
    console_error_panic_hook::set_once();
    let store = Store::new(env.d1("DB")?);
    let summarizer = try_build_summarizer(&env);
    let r2_bucket = env.bucket("RAW_CONTENT").ok();
    let now = (js_sys::Date::now() / 1000.0) as i64;

    // Load rules once per batch
    let rule_jsons = store.active_rule_jsons("default").await.unwrap_or_default();
    let rules: Vec<Rule> = rule_jsons.iter().filter_map(|j| serde_json::from_str(j).ok()).collect();
    let has_rules = !rules.is_empty();

    for msg in batch.messages()?.iter() {
        let job = msg.body();
        console_log!("  queue processing feed {}: {}", job.feed_id, job.feed_url);
        if let Err(e) = process_one_feed(&store, &summarizer, &r2_bucket, &rules, has_rules, job, now).await {
            console_log!("  feed {} failed: {e}", job.feed_id);
            msg.retry();
        } else {
            msg.ack();
        }
    }
    Ok(())
}

/// Cron coordinator: send due feeds to the queue, then run cleanup tasks.
async fn process_all_feeds(env: &Env) -> Result<()> {
    let store = Store::new(env.d1("DB")?);
    let now = (js_sys::Date::now() / 1000.0) as i64;

    // Enqueue due feeds
    let feeds = store
        .feeds_due_for_fetch(now, None)
        .await
        .map_err(|e| Error::RustError(e.to_string()))?;

    console_log!("process_all_feeds: {} feeds due, sending to queue", feeds.len());

    // Try to send via queue; fall back to sync processing if queue isn't configured.
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
        // Queue not available — process synchronously (dev/fallback path).
        console_log!("  FETCH_QUEUE not bound, processing synchronously");
        let summarizer = try_build_summarizer(env);
        let r2_bucket = env.bucket("RAW_CONTENT").ok();
        let rule_jsons = store.active_rule_jsons("default").await.unwrap_or_default();
        let rules: Vec<Rule> = rule_jsons.iter().filter_map(|j| serde_json::from_str(j).ok()).collect();
        let has_rules = !rules.is_empty();
        for feed in &feeds {
            let job = FetchJob {
                feed_id: feed.id,
                feed_url: feed.url.clone(),
                prior_etag: feed.etag.clone(),
                prior_last_modified: feed.last_modified.clone(),
                extraction_level: feed.extraction_level.clone(),
            };
            if let Err(e) = process_one_feed(&store, &summarizer, &r2_bucket, &rules, has_rules, &job, now).await {
                console_log!("  sync feed {} failed: {e}", feed.id);
            }
        }
    }

    // Archive articles older than 30 days
    if let Err(e) = store.expire_old_articles(now, 30).await {
        console_log!("expire_old_articles failed: {e}");
    } else {
        console_log!("article cleanup complete");
    }

    Ok(())
}

/// Process a single feed: fetch → insert → score → AI summarise →
/// optional full-text extraction → optional R2 storage.
async fn process_one_feed(
    store: &Store,
    summarizer: &Option<HttpSummarizer>,
    r2_bucket: &Option<Bucket>,
    rules: &[Rule],
    has_rules: bool,
    job: &FetchJob,
    now: i64,
) -> Result<(), Error> {
    let do_ai = summarizer.is_some();

    match fetch_feed(&job.feed_url, job.prior_etag.as_deref(), job.prior_last_modified.as_deref()).await {
        Ok(FetchOutcome::NotModified) => {
            if let Err(e) = store.record_fetch_result(job.feed_id, now, None, None).await {
                console_log!("    record_fetch_result failed: {e}");
            }
        }
        Ok(FetchOutcome::Updated(fetched)) => {
            for entry in fetched.feed.entries {
                let feed_summary = extract_body(&entry);
                let mut body = feed_summary.clone();

                let article = NewArticle {
                    feed_id: job.feed_id,
                    guid: entry.id.clone(),
                    title: entry.title.map(|t| t.content).unwrap_or_default(),
                    url: entry.links.first().map(|l| l.href.clone()),
                    published_at: entry.published.map(|d| d.timestamp()).filter(|&ts| ts <= now),
                    raw_content_r2_key: None,
                };

                match store.insert_article(&article).await {
                    Ok(Some(article_id)) => {
                        let article_score = if has_rules {
                            score(&ArticleInput { title: &article.title, summary: &body, feed_url: &job.feed_url }, rules, "default")
                        } else {
                            0.0
                        };

                        // Full-text extraction (opt-in per feed)
                        if job.extraction_level == "full_text" {
                            if let Some(ref url) = article.url {
                                match fetcher::extract_full_text(url).await {
                                    Ok(full_text) => {
                                        // Store raw content in R2
                                        let r2_key = format!("articles/{article_id}");
                                        if let Some(ref bucket) = r2_bucket {
                                            let _ = bucket.put(&r2_key, full_text.as_bytes().to_vec()).execute().await;
                                            let _ = store.set_raw_content_r2_key(article_id, Some(&r2_key)).await;
                                        }
                                        // Use full text for AI pipeline
                                        body = full_text;
                                    }
                                    Err(e) => {
                                        console_log!("    full-text extraction failed for article {article_id}: {e}");
                                    }
                                }
                            }
                        }

                        if do_ai {
                            if let Some(ref s) = summarizer {
                                if process_article(store, s, article_id, &article.title, &body, article_score).await.is_err() {
                                    let excerpt = if body.len() > 500 { &body[..500] } else { &body };
                                    let _ = store.set_raw_content_r2_key(article_id, Some(excerpt)).await;
                                }
                            }
                        } else if article_score != 0.0 {
                            let _ = store.set_ai_summary(article_id, "", "[]", &format!("article-{article_id}"), article_score).await;
                        }
                    }
                    Ok(None) => {} // duplicate
                    Err(e) => console_log!("    insert_article failed: {e}"),
                }
            }
            let _ = store.record_fetch_result(job.feed_id, now, fetched.etag.as_deref(), fetched.last_modified.as_deref()).await;
        }
        Err(e) => {
            console_log!("    fetch_feed failed: {e}");
            if !e.is_transient() {
                let _ = store.record_fetch_result(job.feed_id, now, None, None).await;
            }
        }
    }

    Ok(())
}

fn try_build_summarizer(env: &Env) -> Option<HttpSummarizer> {
    let api_key = match env.secret("AI_API_KEY") {
        Ok(v) => v.to_string(),
        Err(_) => { console_log!("AI_API_KEY not set"); return None; }
    };
    let base_url = env.var("AI_BASE_URL").ok().map(|v| v.to_string()).unwrap_or_else(|| "https://api.deepseek.com/v1".into());
    let chat_model = env.var("AI_CHAT_MODEL").ok().map(|v| v.to_string()).unwrap_or_else(|| "deepseek-v4-flash".into());
    let embedding_model = env.var("AI_EMBEDDING_MODEL").ok().map(|v| v.to_string()).unwrap_or_default();
    Some(HttpSummarizer::new(base_url, api_key, chat_model, embedding_model))
}

fn extract_body(entry: &feed_rs::model::Entry) -> String {
    entry.summary.as_ref().map(|s| s.content.clone())
        .or_else(|| entry.content.as_ref().and_then(|c| c.body.clone()))
        .or_else(|| {
            let texts: Vec<&str> = entry.media.iter().filter_map(|m| m.description.as_ref().map(|d| d.content.as_str())).collect();
            if texts.is_empty() { None } else { Some(texts.join("\n")) }
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
