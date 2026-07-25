use serde::Serialize;
use worker::*;
use worker::wasm_bindgen::JsValue;

use crate::version::PIPELINE_VERSION;
use ai_pipeline::briefing::{generate_daily_brief, SignalCandidate};
use store::{SignalThreadFilter, Store};

/// Provenance metadata recorded alongside each generated briefing.
#[derive(Serialize)]
struct BriefingProvenance {
    pipeline_version: String,
    generated_at: i64,
    signal_count: u32,
    insight_count: u32,
    total_signals_loaded: u32,
}

/// Generate today's intelligence briefing and persist it.
///
/// Guarded by a KV lock (`briefing_lock:YYYY-MM-DD`, TTL 1h) so only
/// the first cron cycle of the day creates the briefing.  Subsequent
/// cycles find the lock and skip.
pub(crate) async fn generate_briefing_task(env: &Env, now: i64) {
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

    // 1. Load active signal threads via filter-based query
    let filter = SignalThreadFilter {
        statuses: vec!["active".into(), "decaying".into()],
        limit: 30,
        min_score: 0.0,
    };
    let thread_inputs = match store.list_signal_threads(&filter).await {
        Ok(s) => s,
        Err(e) => {
            console_log!("[Sulix:briefing] list_signal_threads failed: {e}");
            return;
        }
    };
    if thread_inputs.is_empty() {
        console_log!("[Sulix:briefing] no active signal threads — skipping");
        return;
    }

    // 2. Convert to SignalCandidate via the From<SignalBriefInput> converter
    let candidates: Vec<SignalCandidate> = thread_inputs.into_iter().map(Into::into).collect();
    let total_signals_loaded = candidates.len() as u32;

    // 3. Build summarizer
    let summarizer = match crate::services::summarizer::try_build_summarizer(env) {
        Some(s) => s,
        None => {
            console_log!("[Sulix:briefing] no AI summarizer available — skipping");
            return;
        }
    };

    // 4. Lifecycle evaluation (signals already persisted in signal_threads table)
    if let Err(e) = store.update_signal_lifecycle(now).await {
        console_log!("[Sulix:briefing] lifecycle update failed: {e}");
    }

    // 5. Acquire lock before generating (prevent concurrent cron runs)
    let cache = env.kv("CACHE").ok();
    if let Some(ref cache) = cache {
        if let Ok(pb) = cache.put(&lock_key, "1") {
            let _ = pb.expiration_ttl(3600).execute().await;
        }
    }

    // 6. Generate
    let briefing = match generate_daily_brief(candidates, &summarizer, &date, now).await {
        Ok(b) => b,
        Err(e) => {
            console_log!("[Sulix:briefing] generation failed: {e}");
            return;
        }
    };

    // 7. Persist to D1
    let content = serde_json::to_string(&briefing).unwrap_or_default();
    if let Err(e) = store.save_briefing(&date, now, briefing.signal_count, &content).await {
        console_log!("[Sulix:briefing] D1 save failed: {e}");
        return;
    }

    // 8. Write KV cache
    if let Some(ref cache) = cache {
        let cache_key = format!("briefing:{date}");
        if let Ok(pb) = cache.put(&cache_key, &content) {
            let _ = pb.expiration_ttl(21600).execute().await;
        }
    }

    // 9. Save provenance alongside the briefing
    let provenance = BriefingProvenance {
        pipeline_version: PIPELINE_VERSION.into(),
        generated_at: now,
        signal_count: briefing.signal_count,
        insight_count: briefing.insights.len() as u32,
        total_signals_loaded,
    };
    if let Some(ref cache) = cache {
        let prov_key = format!("briefing_provenance:{date}");
        if let Ok(pb) = cache.put(&prov_key, serde_json::to_string(&provenance).unwrap_or_default()) {
            let _ = pb.expiration_ttl(21600).execute().await;
        }
    }

    console_log!(
        "[Sulix:briefing] generated for {date} — {} insights",
        briefing.insights.len()
    );
}