use serde::{Deserialize, Serialize};
use worker::*;

use ai_pipeline::{process_article, HttpSummarizer};
use api::router;
use fetcher::{extract_full_text, fetch_feed};
use rules::{score, ArticleInput, Rule};
use store::{NewArticle, Store};

/// One message per feed to fetch.  Now carries extraction_level so the
/// consumer can decide per-article whether to run full-text fetching.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FetchJob {
    feed_id: i64,
    feed_url: String,
    prior_etag: Option<String>,
    prior_last_modified: Option<String>,
    extraction_level: String,
}

/// HTTP entry point.
#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();
    router().run(req, env).await
}

/// Producer side.  Runs on the Cron Trigger; reads only feeds that are
/// due for a fetch (respecting fetch_interval_sec) and enqueues one
/// message per feed.  No fetch work happens here.
#[event(scheduled)]
async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    console_error_panic_hook::set_once();
    if let Err(e) = enqueue_fetch_jobs(&env).await {
        console_log!("enqueue_fetch_jobs failed: {e}");
    }
}

async fn enqueue_fetch_jobs(env: &Env) -> Result<()> {
    let store = Store::new(env.d1("DB")?);
    let queue = env.queue("FETCH_QUEUE")?;
    let now = (js_sys::Date::now() / 1000.0) as i64;

    // Only enqueue feeds whose fetch_interval_sec has elapsed since last_fetched_at.
    // Category filter not used yet (None = all).
    let feeds = store
        .feeds_due_for_fetch(now, None)
        .await
        .map_err(|e| Error::RustError(e.to_string()))?;

    console_log!("enqueue_fetch_jobs: {} feeds due for fetch at {}", feeds.len(), now);

    for feed in feeds {
        let job = FetchJob {
            feed_id: feed.id,
            feed_url: feed.url,
            prior_etag: feed.etag,
            prior_last_modified: feed.last_modified,
            extraction_level: feed.extraction_level,
        };
        if let Err(e) = queue.send(&job).await {
            console_log!("queue.send failed for feed {}: {e}", job.feed_id);
        }
    }

    Ok(())
}

/// Try to build an HttpSummarizer from Worker env vars.
fn try_build_summarizer(env: &Env) -> Option<HttpSummarizer> {
    let api_key = match env.secret("AI_API_KEY") {
        Ok(v) => v.to_string(),
        Err(_) => {
            console_log!("AI_API_KEY not set -- skipping AI summarization");
            return None;
        }
    };
    let base_url = env
        .var("AI_BASE_URL")
        .ok()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "https://api.deepseek.com/v1".into());
    let chat_model = env
        .var("AI_CHAT_MODEL")
        .ok()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "deepseek-v4-flash".into());
    let embedding_model = env
        .var("AI_EMBEDDING_MODEL")
        .ok()
        .map(|v| v.to_string())
        .unwrap_or_default();

    Some(HttpSummarizer::new(base_url, api_key, chat_model, embedding_model))
}

/// Extract readable body text from a feed entry.  Falls back from summary
/// to content to media descriptions.
fn extract_entry_body(entry: &feed_rs::model::Entry) -> String {
    entry
        .summary
        .as_ref()
        .map(|s| s.content.clone())
        .or_else(|| entry.content.as_ref().and_then(|c| c.body.clone()))
        .or_else(|| {
            let texts: Vec<&str> = entry
                .media
                .iter()
                .filter_map(|m| m.description.as_ref().map(|d| d.content.as_str()))
                .collect();
            if texts.is_empty() { None } else { Some(texts.join("\n")) }
        })
        .unwrap_or_default()
}

/// Consumer side.  Bound to `FETCH_QUEUE` in wrangler.toml.
/// After fetching, scores new articles with the rules engine, optionally
/// extracts full text from the article URL (per-feed opt-in via
/// extraction_level), then runs AI summarization.
#[event(queue)]
async fn queue(batch: MessageBatch<FetchJob>, env: Env, _ctx: Context) -> Result<()> {
    console_error_panic_hook::set_once();
    let store = Store::new(env.d1("DB")?);
    let summarizer = try_build_summarizer(&env);
    let r2_bucket = env.bucket("RAW_CONTENT").ok();

    // Load rules once per batch.
    let rule_jsons = store.active_rule_jsons("default").await.unwrap_or_default();
    let rules: Vec<Rule> = rule_jsons.iter().filter_map(|j| serde_json::from_str(j).ok()).collect();
    let has_rules = !rules.is_empty();

    let messages = batch.messages()?;
    for msg in messages.iter() {
        let job = msg.body();
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let do_full_text = job.extraction_level == "full_text";

        match fetch_feed(
            &job.feed_url,
            job.prior_etag.as_deref(),
            job.prior_last_modified.as_deref(),
        )
        .await
        {
            Ok(fetcher::FetchOutcome::NotModified) => {
                if let Err(e) = store.record_fetch_result(job.feed_id, now, None, None).await {
                    console_log!("record_fetch_result failed for feed {}: {e}", job.feed_id);
                }
                msg.ack();
            }
            Ok(fetcher::FetchOutcome::Updated(fetched)) => {
                for entry in fetched.feed.entries {
                    let body = extract_entry_body(&entry);

                    let article = NewArticle {
                        feed_id: job.feed_id,
                        guid: entry.id.clone(),
                        title: entry.title.map(|t| t.content).unwrap_or_default(),
                        url: entry.links.first().map(|l| l.href.clone()),
                        published_at: entry.published.map(|d| d.timestamp()),
                        raw_content_r2_key: None,
                    };

                    match store.insert_article(&article).await {
                        Ok(Some(article_id)) => {
                            // Full-text extraction (per-feed opt-in, SSRF-guarded)
                            let body_for_ai = if do_full_text {
                                if let Some(ref url) = article.url {
                                    match extract_full_text(url).await {
                                        Ok(extracted) => {
                                            // Store extracted text in R2 under a deterministic key.
                                            let r2_key = format!("articles/{}/{}.txt", job.feed_id, article_id);
                                            if let Some(ref bucket) = r2_bucket {
                                                if let Err(e) = bucket.put(&r2_key, extracted.clone()).execute().await {
                                                    console_log!("R2 put failed for {}: {e}", r2_key);
                                                }
                                                // Record the R2 key in the article row regardless of
                                                // whether the bucket put succeeded (best-effort).
                                                let _ = store.set_raw_content_r2_key(article_id, Some(&r2_key)).await;
                                            }
                                            extracted
                                        }
                                        Err(e) => {
                                            if e.is_transient() {
                                                console_log!("extract_full_text transient error for {}: {e} — retrying later", url);
                                            } else {
                                                console_log!("extract_full_text permanent error for {}: {e} — skipping full text", url);
                                            }
                                            body // fall back to RSS body
                                        }
                                    }
                                } else {
                                    body
                                }
                            } else {
                                body
                            };

                            // Score from rules engine.
                            let article_score = if has_rules {
                                let input = ArticleInput {
                                    title: &article.title,
                                    summary: &body_for_ai,
                                    feed_url: &job.feed_url,
                                };
                                score(&input, &rules, "default")
                            } else {
                                0.0
                            };

                            // AI summarization (when API key is available).
                            if let Some(ref s) = summarizer {
                                if let Err(e) = process_article(
                                    &store, s, article_id, &article.title, &body_for_ai, article_score,
                                )
                                .await
                                {
                                    console_log!("AI pipeline failed for article {}: {e}", article_id);
                                }
                            } else if article_score != 0.0 {
                                let _ = store
                                    .set_ai_summary(article_id, "", "[]", &format!("article-{article_id}"), article_score)
                                    .await;
                            }
                        }
                        Ok(None) => { /* duplicate, skip */ }
                        Err(e) => { console_log!("insert_article failed for feed {}: {e}", job.feed_id); }
                    }
                }

                if let Err(e) = store.record_fetch_result(
                    job.feed_id, now, fetched.etag.as_deref(), fetched.last_modified.as_deref(),
                ).await {
                    console_log!("record_fetch_result failed for feed {}: {e}", job.feed_id);
                }
                msg.ack();
            }
            Err(e) => {
                console_log!("fetch_feed failed for {}: {e}", job.feed_url);
                // Only retry transient errors; permanent errors ack+discard.
                if e.is_transient() {
                    msg.retry();
                } else {
                    console_log!("  permanent error — discarding without retry");
                    msg.ack();
                }
            }
        }
    }

    Ok(())
}
