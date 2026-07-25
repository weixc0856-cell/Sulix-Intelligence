//! Signal Engine orchestration — thin cron-job wrapper.
//!
//! Responsibility: when to run, error handling, observability.
//! The actual signal-computation logic lives in `intelligence/signal-engine` crate.

use worker::*;

use signal_engine::source::{EntitySignalSource, SignalSource};
use signal_engine::SignalEngine;
use store::D1Store;

/// Run one cycle of the Signal Engine.
///
/// Designed to be called from the cron handler after feed ingestion
/// and entity persistence are complete, before briefing generation.
pub(crate) async fn run_signal_engine(env: &Env, now: i64) {
    let db = match env.d1("DB") {
        Ok(db) => db,
        Err(e) => {
            console_log!("signal engine: failed to get DB binding: {e}");
            return;
        }
    };
    let store = D1Store::new(db);

    // Build signal sources
    let entity_source = EntitySignalSource;
    let sources: [&dyn SignalSource; 1] = [&entity_source];

    // When SemanticDiscoverySource is ready, add it here:
    // let vz = env.vectorize("VECTORIZE").ok();
    // if let Some(v) = vz {
    //     let discovery = SemanticDiscoverySource { vectorize: &v, embedder: &embedder };
    //     let sources: [&dyn SignalSource; 2] = [&entity_source, &discovery];
    // }

    match SignalEngine::run(&store, &sources, now).await {
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
}
