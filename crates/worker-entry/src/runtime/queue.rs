use std::cell::RefCell;
use std::collections::HashSet;
use worker::*;

pub(crate) use crate::jobs::ingestion::FetchJob;
use crate::jobs::ingestion::{process_one_feed, FeedContext};
use crate::metrics::PipelineMetrics;
use rules::Rule;
use store::Store;
use vectorize::VectorizeIndex;

// ---------------------------------------------------------------------------
// Queue event handler
// ---------------------------------------------------------------------------

pub(crate) async fn handle(batch: MessageBatch<FetchJob>, env: Env, _ctx: Context) -> Result<()> {
    console_error_panic_hook::set_once();
    let store = Store::new(env.d1("DB")?);
    let summarizer = crate::services::summarizer::try_build_summarizer(&env);
    let embedder = crate::services::embedder::try_build_embedder(&env);
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
        embedder: &embedder,
        r2_bucket: &r2_bucket,
        vectorize: &vectorize,
        rules: &rules,
        has_rules: !rules.is_empty(),
        now,
        metrics,
    };
    for msg in batch.messages()?.iter() {
        let job = msg.body();
        // Pre-load already-ingested guids so process_one_feed can skip them
        // without a per-entry query (keeps large feeds under the per-invocation
        // D1 subrequest cap).
        let existing_guids: HashSet<String> = match store.guids_for_feed(job.feed_id).await {
            Ok(g) => g.into_iter().collect(),
            Err(e) => {
                console_log!("  failed to load existing guids for feed {}: {e}; retrying", job.feed_id);
                msg.retry();
                continue;
            }
        };
        console_log!("  queue processing feed {}: {}", job.feed_id, job.feed_url);
        if let Err(e) = process_one_feed(&feed_ctx, &env, job, &existing_guids).await {
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
