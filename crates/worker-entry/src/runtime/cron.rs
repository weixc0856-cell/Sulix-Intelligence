use worker::*;

use crate::jobs::{archive, briefing, gc, ingestion, signal};

pub(crate) async fn handle(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    console_error_panic_hook::set_once();
    console_log!("scheduled handler at ts={}", js_sys::Date::now());
    if let Err(e) = ingestion::process_all_feeds(&env).await {
        console_log!("scheduled handler: feed ingestion failed: {e}");
    }
    // R2 garbage collection
    let now = (js_sys::Date::now() / 1000.0) as i64;
    if let Err(e) = gc::gc_r2_objects(&env, now).await {
        console_log!("gc_r2_objects failed: {e}");
    }
    // Signal Engine — materialise entity candidates into signal threads,
    // append instances, write timeline events, and run lifecycle transitions.
    // Runs every cron cycle and is intentionally before briefing generation
    // so the briefing always sees the latest signal state.
    signal::run_signal_engine(&env, now).await;
    // Daily Intelligence Brief generation — runs once per day (KV lock).
    briefing::generate_briefing_task(&env, now).await;

    // Object Outbox — drain pending archive entries to the R2 Memory Archive.
    // Runs last so all artifacts from the current cycle are included.
    archive::archive_outbox(&env).await;
}
