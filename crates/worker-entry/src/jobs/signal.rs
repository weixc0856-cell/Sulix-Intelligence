//! Signal Engine orchestration — thin cron-job wrapper.
//!
//! Responsibility: when to run, error handling, observability.
//!
//! Incremental checkpoint: uses KV `signal_engine:last_run` to skip
//! processing when no new articles have been ingested since the last
//! engine cycle.  This is the primary D1 write-amplification guard for
//! the intelligence pipeline — every cron cycle would otherwise create
//! a fresh round of signal instances + events even when nothing changed.

use event_store::{EventR2Backend, EventStore};
use infrastructure::semantic_query::VectorizeSemanticQuery;
use object_store::R2Store;
use worker::*;

use signal_engine::ports::SemanticQuery;
use signal_engine::source::{EntitySignalSource, SemanticDiscoverySource, SignalSource};
use signal_engine::SignalEngine;
use store::D1Store;

/// KV key for the engine checkpoint cursor.
const KV_LAST_RUN: &str = "signal_engine:last_run";

/// Run one cycle of the Signal Engine, skipping when no new articles exist.
pub(crate) async fn run_signal_engine(env: &Env, now: i64) {
    // ---- Incremental checkpoint via KV + D1 ----
    let cache = env.kv("CACHE").ok();
    let db = env.d1("DB").ok();

    if let (Some(ref cache), Some(ref db)) = (cache, db) {
        let last_ts: Option<i64> = match cache.get(KV_LAST_RUN).text().await {
            Ok(Some(val)) => val.trim().parse::<i64>().ok(),
            _ => None,
        };

        if let Some(ts) = last_ts {
            let stmt = db.prepare("SELECT COUNT(*) AS cnt FROM articles WHERE created_at > ?1");
            let count = match stmt.bind(&[wasm_bindgen::JsValue::from_f64(ts as f64)]) {
                Ok(stmt) => match stmt.first::<serde_json::Value>(None).await {
                    Ok(Some(row)) => row["cnt"].as_i64().unwrap_or(0),
                    _ => 0,
                },
                _ => 0,
            };
            if count == 0 {
                console_log!("signal engine: no new articles since ts={} — skipping", ts);
                return;
            }
        }
    }

    // ---- Full engine cycle ----
    let db = match env.d1("DB") {
        Ok(db) => db,
        Err(e) => {
            console_log!("signal engine: failed to get DB binding: {e}");
            return;
        }
    };
    let store = D1Store::new(db);

    let entity_source = EntitySignalSource;
    let semantic_source = SemanticDiscoverySource;

    let semantic: Option<VectorizeSemanticQuery> = env.get_binding("VECTORIZE").ok().map(VectorizeSemanticQuery::new);
    if semantic.is_some() {
        console_log!("semantic discovery: enabled");
    }
    let semantic_ref: Option<&dyn SemanticQuery> = semantic.as_ref().map(|s| s as &dyn SemanticQuery);

    let sources: [&dyn SignalSource; 2] = [&entity_source, &semantic_source];

    // Build EventStore (R2 archive + D1 outbox/index)
    let event_store: Option<EventR2Backend<D1Store>> = match (env.d1("DB").ok(), env.bucket("RAW_CONTENT").ok()) {
        (Some(db), Some(bucket)) => Some(EventR2Backend::new(D1Store::new(db), R2Store::new(bucket))),
        _ => None,
    };
    let event_store_ref: Option<&dyn EventStore> = event_store.as_ref().map(|es| es as &dyn EventStore);

    match SignalEngine::run(&store, event_store_ref, semantic_ref, &sources, now).await {
        Ok(report) => {
            console_log!(
                "signal engine: {} threads, {} instances, {} events, {} lifecycle transitions",
                report.threads_created,
                report.instances_appended,
                report.events_written,
                report.lifecycle_transitions,
            );
        }
        Err(e) => {
            console_log!("signal engine: run failed: {e}");
        }
    }

    // ---- Persist checkpoint ----
    if let Ok(cache) = env.kv("CACHE") {
        if let Ok(pb) = cache.put(KV_LAST_RUN, now.to_string()) {
            let _ = pb.expiration_ttl(604_800).execute().await; // 7-day TTL
        }
    }
}
