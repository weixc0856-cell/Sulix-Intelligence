//! Signal Engine orchestration — thin cron-job wrapper.
//!
//! Responsibility: when to run, error handling, observability.

use worker::*;

use signal_engine::source::{EntitySignalSource, SemanticDiscoverySource, SignalSource};
use signal_engine::SignalEngine;
use store::D1Store;
use vectorize::VectorizeIndex;

/// Run one cycle of the Signal Engine.
pub(crate) async fn run_signal_engine(env: &Env, now: i64) {
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

    let vz: Option<VectorizeIndex> = env.get_binding("VECTORIZE").ok();
    if vz.is_some() {
        console_log!("semantic discovery: enabled");
    }

    let sources: [&dyn SignalSource; 2] = [&entity_source, &semantic_source];

    match SignalEngine::run(&store, vz.as_ref(), &sources, now).await {
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
