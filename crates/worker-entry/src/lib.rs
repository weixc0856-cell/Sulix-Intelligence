use serde::{Deserialize, Serialize};
use worker::*;

use ai_pipeline::{process_article, HttpSummarizer};
use api::router;
use fetcher::fetch_feed;
use rules::{score, ArticleInput, Rule};
use store::{NewArticle, Store};

/// One message per feed to fetch. Carries the prior etag/last_modified so
/// the consumer can send a conditional request without a separate D1 read.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FetchJob {
    feed_id: i64,
    feed_url: String,
    prior_etag: Option<String>,
    prior_last_modified: Option<String>,
}

/// HTTP entry point. D1 is bound in wrangler.toml as `DB`.
#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();
    router().run(req, env).await
}

/// Producer side of the fetch pipeline. Runs on the Cron Trigger, does NOT
/// fetch anything itself -- it just reads the active feed list and drops
/// one small message per feed onto the `FETCH_QUEUE`.
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

    let feeds = store
        .active_feeds()
        .await
        .map_err(|e| Error::RustError(e.to_string()))?;

    for feed in feeds {
        let job = FetchJob {
            feed_id: feed.id,
            feed_url: feed.url,
            prior_etag: feed.etag,
            prior_last_modified: feed.last_modified,
        };
        if let Err(e) = queue.send(&job).await {
            console_log!("queue.send failed for feed {}: {e}", job.feed_id);
        }
    }

    Ok(())
}

/// Try to build an HttpSummarizer from Worker env vars. Returns None when
/// AI_API_KEY is not set, so the pipeline degrades gracefully to
/// fetch-and-store without AI processing.
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
        .unwrap_or_else(|| "https://api.openai.com/v1".into());
    let chat_model = env
        .var("AI_CHAT_MODEL")
        .ok()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "gpt-4o-mini".into());
    let embedding_model = env
        .var("AI_EMBEDDING_MODEL")
        .ok()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "text-embedding-3-small".into());

    Some(HttpSummarizer::new(base_url, api_key, chat_model, embedding_model))
}

/// Extract readable body text from a feed entry. Prefers summary over
/// full content, falls back to empty string.
fn extract_body(entry: &feed_rs::model::Entry) -> String {
    entry
        .summary
        .as_ref()
        .map(|s| s.content.clone())
        .or_else(|| {
            entry
                .content
                .as_ref()
                .and_then(|c| c.body.clone())
        })
        .or_else(|| {
            // Last resort: concat all media description texts
            let texts: Vec<&str> = entry
                .media
                .iter()
                .filter_map(|m| m.description.as_ref().map(|d| d.content.as_str()))
                .collect();
            if texts.is_empty() {
                None
            } else {
                Some(texts.join("\n"))
            }
        })
        .unwrap_or_default()
}

/// Consumer side. Bound to `FETCH_QUEUE` in wrangler.toml; Cloudflare
/// invokes this per batch of messages, retrying failed ones automatically.
/// After fetching, scores each new article with the rules engine and
/// enriches it with AI-generated summary, tags, and embedding.
#[event(queue)]
async fn queue(batch: MessageBatch<FetchJob>, env: Env, _ctx: Context) -> Result<()> {
    console_error_panic_hook::set_once();
    let store = Store::new(env.d1("DB")?);
    let summarizer = try_build_summarizer(&env);

    // Load rules once per batch (they change infrequently)
    let rule_jsons = store
        .active_rule_jsons("default")
        .await
        .unwrap_or_default();
    let rules: Vec<Rule> = rule_jsons
        .iter()
        .filter_map(|j| serde_json::from_str(j).ok())
        .collect();
    let has_rules = !rules.is_empty();

    let messages = batch.messages()?;
    for msg in messages.iter() {
        let job = msg.body();
        let now = (js_sys::Date::now() / 1000.0) as i64;

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
                    let body = extract_body(&entry);
                    let article = NewArticle {
                        feed_id: job.feed_id,
                        guid: entry.id,
                        title: entry.title.map(|t| t.content).unwrap_or_default(),
                        url: entry.links.first().map(|l| l.href.clone()),
                        published_at: entry.published.map(|d| d.timestamp()),
                        raw_content_r2_key: None,
                    };

                    match store.insert_article(&article).await {
                        Ok(Some(article_id)) => {
                            // Compute score from rules
                            let article_score = if has_rules {
                                let input = ArticleInput {
                                    title: &article.title,
                                    summary: &body,
                                    feed_url: &job.feed_url,
                                };
                                score(&input, &rules, "default")
                            } else {
                                0.0
                            };

                            // Run AI summarization when available
                            if let Some(ref s) = summarizer {
                                if let Err(e) = process_article(
                                    &store,
                                    s,
                                    article_id,
                                    &article.title,
                                    &body,
                                    article_score,
                                )
                                .await
                                {
                                    console_log!(
                                        "AI pipeline failed for article {}: {e}",
                                        article_id
                                    );
                                }
                            } else if article_score != 0.0 {
                                // No AI but we have a score -- persist it
                                if let Err(e) = store
                                    .set_ai_summary(
                                        article_id,
                                        "",
                                        "[]",
                                        &format!("article-{article_id}"),
                                        article_score,
                                    )
                                    .await
                                {
                                    console_log!(
                                        "set_ai_summary failed for {}: {e}",
                                        article_id
                                    );
                                }
                            }
                        }
                        Ok(None) => { /* duplicate row, skip */ }
                        Err(e) => {
                            console_log!("insert_article failed for feed {}: {e}", job.feed_id);
                        }
                    }
                }
                if let Err(e) = store
                    .record_fetch_result(
                        job.feed_id,
                        now,
                        fetched.etag.as_deref(),
                        fetched.last_modified.as_deref(),
                    )
                    .await
                {
                    console_log!("record_fetch_result failed for feed {}: {e}", job.feed_id);
                }
                msg.ack();
            }
            Err(e) => {
                console_log!("fetch_feed failed for {}: {e}", job.feed_url);
                msg.retry();
            }
        }
    }

    Ok(())
}
