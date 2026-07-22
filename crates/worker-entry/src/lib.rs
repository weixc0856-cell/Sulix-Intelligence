use serde::{Deserialize, Serialize};
use worker::*;

use api::router;
use fetcher::fetch_feed;
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
    router().run(req, env).await
}

/// Producer side of the fetch pipeline. Runs on the Cron Trigger, does NOT
/// fetch anything itself -- it just reads the active feed list and drops
/// one small message per feed onto the `FETCH_QUEUE`.
#[event(scheduled)]
async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
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

/// Consumer side. Bound to `FETCH_QUEUE` in wrangler.toml; Cloudflare
/// invokes this per batch of messages, retrying failed ones automatically.
#[event(queue)]
async fn queue(batch: MessageBatch<FetchJob>, env: Env, _ctx: Context) -> Result<()> {
    let store = Store::new(env.d1("DB")?);

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
                    let article = NewArticle {
                        feed_id: job.feed_id,
                        guid: entry.id,
                        title: entry.title.map(|t| t.content).unwrap_or_default(),
                        url: entry.links.first().map(|l| l.href.clone()),
                        published_at: entry.published.map(|d| d.timestamp()),
                        raw_content_r2_key: None,
                    };
                    if let Err(e) = store.insert_article(&article).await {
                        console_log!("insert_article failed for feed {}: {e}", job.feed_id);
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
