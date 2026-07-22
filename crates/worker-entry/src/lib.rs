use serde::{Deserialize, Serialize};
use worker::*;

use ai_pipeline::{process_article, HttpSummarizer};
use api::router;
use fetcher::{extract_full_text, fetch_feed, FetchError, FetchOutcome};
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

/// Cron handler: process all due feeds directly (no queue).
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

/// Core pipeline: iterate due feeds, fetch each, insert articles, score, AI-summarize.
async fn process_all_feeds(env: &Env) -> Result<()> {
    let store = Store::new(env.d1("DB")?);
    let summarizer = try_build_summarizer(env);
    let r2_bucket = env.bucket("RAW_CONTENT").ok();
    let now = (js_sys::Date::now() / 1000.0) as i64;

    // Load rules once
    let rule_jsons = store.active_rule_jsons("default").await.unwrap_or_default();
    let rules: Vec<Rule> = rule_jsons.iter().filter_map(|j| serde_json::from_str(j).ok()).collect();
    let has_rules = !rules.is_empty();

    let feeds = store.feeds_due_for_fetch(now, None)
        .await
        .map_err(|e| Error::RustError(e.to_string()))?;

    console_log!("process_all_feeds: {} feeds due", feeds.len());

    for feed in feeds {
        console_log!("  processing feed {}: {}", feed.id, feed.title.as_deref().unwrap_or("?"));
        let do_ai = summarizer.is_some();

        match fetch_feed(&feed.url, feed.etag.as_deref(), feed.last_modified.as_deref()).await {
            Ok(FetchOutcome::NotModified) => {
                if let Err(e) = store.record_fetch_result(feed.id, now, None, None).await {
                    console_log!("    record_fetch_result failed: {e}");
                }
            }
            Ok(FetchOutcome::Updated(fetched)) => {
                for entry in fetched.feed.entries {
                    let body = extract_body(&entry);
                    let article = NewArticle {
                        feed_id: feed.id,
                        guid: entry.id.clone(),
                        title: entry.title.map(|t| t.content).unwrap_or_default(),
                        url: entry.links.first().map(|l| l.href.clone()),
                        published_at: entry.published.map(|d| d.timestamp()),
                        raw_content_r2_key: None,
                    };

                    match store.insert_article(&article).await {
                        Ok(Some(article_id)) => {
                            let article_score = if has_rules {
                                score(&ArticleInput { title: &article.title, summary: &body, feed_url: &feed.url }, &rules, "default")
                            } else { 0.0 };

                            if do_ai {
                                if let Some(ref s) = summarizer {
                                    if let Err(e) = process_article(&store, s, article_id, &article.title, &body, article_score).await {
                                        console_log!("    AI pipeline failed for {}: {e}", article_id);
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
                let _ = store.record_fetch_result(feed.id, now, fetched.etag.as_deref(), fetched.last_modified.as_deref()).await;
            }
            Err(e) => {
                console_log!("    fetch_feed failed: {e}");
                if !e.is_transient() {
                    let _ = store.record_fetch_result(feed.id, now, None, None).await;
                }
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

/// Queue consumer (kept for future async fan-out, currently not used).
#[event(queue)]
async fn queue(batch: MessageBatch<FetchJob>, env: Env, _ctx: Context) -> Result<()> {
    console_error_panic_hook::set_once();
    console_log!("queue consumer invoked with {} messages", batch.messages()?.len());
    for msg in batch.messages()?.iter() {
        msg.ack();
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FetchJob {
    feed_id: i64,
    feed_url: String,
    prior_etag: Option<String>,
    prior_last_modified: Option<String>,
    extraction_level: String,
}
