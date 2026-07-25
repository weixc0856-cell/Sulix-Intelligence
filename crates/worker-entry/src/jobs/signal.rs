//! Signal Engine orchestration — thin cron-job wrapper.
//!
//! This is the production entry point for the signal engine.
//! Responsibility: when to run, error handling, observability.
//!
//! The actual signal-computation logic lives in
//! `intelligence/signal-engine` crate.

use worker::*;

use signal_engine::SignalEngine;
use store::D1Store;

/// Run one cycle of the Signal Engine.
///
/// Designed to be called from the cron handler **after** feed ingestion
/// and entity persistence are complete, and **before** briefing generation.
pub(crate) async fn run_signal_engine(env: &Env, now: i64) {
    let db = match env.d1("DB") {
        Ok(db) => db,
        Err(e) => {
            console_log!("signal engine: failed to get DB binding: {e}");
            return;
        }
    };
    let store = D1Store::new(db);

    match SignalEngine::run(&store, now).await {
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
