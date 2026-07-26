use serde::Serialize;
use worker::wasm_bindgen::JsValue;
use worker::*;

use crate::version::PIPELINE_VERSION;
use ai_pipeline::briefing::{generate_daily_brief, SignalCandidate};
use object_store::{ObjectStore, R2Store};
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

/// Envelope stored in the R2 Memory Archive for immutable daily briefings.
#[derive(Serialize)]
struct BriefingArtifactEnvelope {
    schema_version: i32,
    artifact_type: String,
    date: String,
    content: serde_json::Value,
    metadata: BriefingArtifactMetadata,
    created_at: i64,
}

#[derive(Serialize)]
struct BriefingArtifactMetadata {
    signal_count: u32,
    insight_count: u32,
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
    let object_store: Option<R2Store> = env.bucket("RAW_CONTENT").ok().map(R2Store::new);
    let cache = env.kv("CACHE").ok();

    // 1. Load active signal threads via filter-based query
    let filter = SignalThreadFilter { statuses: vec!["active".into(), "decaying".into()], limit: 30, min_score: 0.0 };
    let thread_inputs = match store.list_signal_threads(&filter).await {
        Ok(s) => s,
        Err(e) => {
            console_log!("[Sulix:briefing] list_signal_threads failed: {e}");
            return;
        }
    };
    if thread_inputs.is_empty() {
        // Fallback to legacy signals_today() — retains compatibility during
        // the transition period (1-2 weeks) while the Signal Engine builds
        // its first signal threads from historical data.
        console_log!("[Sulix:briefing] no signal threads — falling back to signals_today()");
        let candidates = match fallback_signals_today(&store, now).await {
            Some(c) => c,
            None => {
                console_log!("[Sulix:briefing] no signals available — skipping");
                return;
            }
        };
        // Skip lifecycle update in fallback mode
        let total_signals_loaded = candidates.len() as u32;

        // Build summarizer (shared logic continues below)
        let summarizer = match crate::services::summarizer::try_build_summarizer(env) {
            Some(s) => s,
            None => {
                console_log!("[Sulix:briefing] no AI summarizer available — skipping");
                return;
            }
        };

        // Acquire lock
        if let Some(ref cache) = cache {
            if let Ok(pb) = cache.put(&lock_key, "1") {
                let _ = pb.expiration_ttl(3600).execute().await;
            }
        }

        // Generate, persist, cache
        let obj_ref = object_store.as_ref().map(|r| r as &dyn ObjectStore);
        if let Err(e) =
            generate_and_persist(&store, &cache, candidates, &summarizer, &date, now, total_signals_loaded, obj_ref).await
        {
            console_log!("[Sulix:briefing] fallback generation failed: {e}");
        }
        return;
    }

    // V2 path: convert signal threads to candidates with context
    let thread_ids: Vec<i64> = thread_inputs.iter().map(|t| t.thread_id).collect();
    let context_bundle = store.get_signal_briefing_context_bundle(&thread_ids).await.unwrap_or_default();
    let candidates: Vec<SignalCandidate> = thread_inputs
        .into_iter()
        .map(|input| {
            let tid = input.thread_id;
            let mut candidate: SignalCandidate = input.into();
            if let Some(decisions) = context_bundle.decision_map.get(&tid) {
                candidate.context.decisions = decisions
                    .iter()
                    .map(|d| ai_pipeline::briefing::context::DecisionContext {
                        id: d.id,
                        title: d.title.clone(),
                        status: d.status.clone(),
                        latest_evaluation: context_bundle.evaluation_map.get(&d.id).cloned().unwrap_or(None),
                    })
                    .collect();
            }
            candidate
        })
        .collect();
    let total_signals_loaded = candidates.len() as u32;

    // 3-5. Build summarizer + lifecycle + lock (same for both paths)
    let summarizer = match crate::services::summarizer::try_build_summarizer(env) {
        Some(s) => s,
        None => {
            console_log!("[Sulix:briefing] no AI summarizer available — skipping");
            return;
        }
    };

    if let Err(e) = store.update_signal_lifecycle(now).await {
        console_log!("[Sulix:briefing] lifecycle update failed: {e}");
    }

    if let Some(ref cache) = cache {
        if let Ok(pb) = cache.put(&lock_key, "1") {
            let _ = pb.expiration_ttl(3600).execute().await;
        }
    }

    // 6-9. Generate + persist + cache + provenance
    let obj_ref = object_store.as_ref().map(|r| r as &dyn ObjectStore);
    if let Err(e) =
        generate_and_persist(&store, &cache, candidates, &summarizer, &date, now, total_signals_loaded, obj_ref).await
    {
        console_log!("[Sulix:briefing] generation failed: {e}");
    }
}

/// Generate a briefing, persist to D1 and R2, cache to KV, and log provenance.
///
/// Shared between the V2 (signal thread) and fallback (legacy signals_today) paths.
async fn generate_and_persist(
    store: &store::Store,
    cache: &Option<worker::kv::KvStore>,
    candidates: Vec<SignalCandidate>,
    summarizer: &ai_pipeline::HttpSummarizer,
    date: &str,
    now: i64,
    total_signals_loaded: u32,
    object_store: Option<&dyn ObjectStore>,
) -> Result<(), String> {
    let briefing =
        generate_daily_brief(candidates, summarizer, date, now).await.map_err(|e| format!("generation failed: {e}"))?;

    let content = serde_json::to_string(&briefing).map_err(|e| format!("serialisation: {e}"))?;

    // 1. R2 Memory Archive — write artifact envelope (canonical source)
    if let Some(os) = object_store {
        let envelope = serde_json::to_string(&BriefingArtifactEnvelope {
            schema_version: 1,
            artifact_type: "daily_briefing".into(),
            date: date.to_string(),
            content: serde_json::to_value(&briefing).map_err(|e| format!("envelope serialisation: {e}"))?,
            metadata: BriefingArtifactMetadata {
                signal_count: briefing.signal_count,
                insight_count: briefing.insights.len() as u32,
            },
            created_at: now,
        })
        .map_err(|e| format!("envelope serialisation: {e}"))?;

        let r2_key = object_store::keys::briefing(date);
        if let Err(e) = os.write_object(&r2_key, envelope.as_bytes()).await {
            // Non-fatal: D1 fallback still works for reads
            console_log!("[Sulix:briefing] R2 write failed for {date}: {e}");
        } else {
            // D1 metadata index (non-fatal on failure)
            if let Err(e) = store
                .put_artifact(&store::NewArtifactRecord {
                    artifact_type: "daily_briefing".into(),
                    artifact_date: date.to_string(),
                    object_key: r2_key,
                    schema_version: 1,
                    content_hash: None,
                    size_bytes: Some(envelope.len() as i64),
                    metadata: Some(
                        serde_json::json!({
                            "signal_count": briefing.signal_count,
                            "insight_count": briefing.insights.len(),
                        })
                        .to_string(),
                    ),
                })
                .await
            {
                console_log!("[Sulix:briefing] artifact index write failed for {date}: {e}");
            }
        }
    }

    // 2. D1 legacy persistence (intelligence_briefs.content — transitional)
    // Removed in Sprint 5.9 — R2 + memory_artifacts is canonical.
    // store.save_briefing(date, now, briefing.signal_count, &content).await.map_err(|e| format!("D1 save: {e}"))?;

    if let Some(ref cache) = cache {
        let cache_key = format!("briefing:{date}");
        if let Ok(pb) = cache.put(&cache_key, &content) {
            let _ = pb.expiration_ttl(21600).execute().await;
        }
    }

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

    console_log!("[Sulix:briefing] generated for {date} — {} insights", briefing.insights.len());
    Ok(())
}

/// Fallback: load legacy `signals_today()` candidates when no signal threads exist.
///
/// This bridges the transition period (1-2 weeks) while the Signal Engine builds
/// its first signal threads from historical entity activity.
async fn fallback_signals_today(store: &store::Store, now: i64) -> Option<Vec<SignalCandidate>> {
    let today_signals = store.signals_today(now).await.ok()?;
    if today_signals.is_empty() {
        return None;
    }

    let candidates: Vec<SignalCandidate> = today_signals
        .into_iter()
        .map(|s| SignalCandidate {
            id: s.id,
            title: s.title,
            category: String::new(),
            signal_summary: s.summary,
            article_count: s.articles.len(),
            source_count: 1,
            avg_score: s.confidence,
            trend: s.trend,
            articles: s
                .articles
                .into_iter()
                .map(|a| ai_pipeline::briefing::EvidenceArticle {
                    id: a.id,
                    title: a.title,
                    url: a.url,
                    feed_name: a.feed_name,
                    score: a.score,
                })
                .collect(),
            context: Default::default(),
        })
        .collect();

    if candidates.is_empty() {
        return None;
    }
    Some(candidates)
}
