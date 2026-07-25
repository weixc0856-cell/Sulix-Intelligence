use worker::*;

use crate::jobs::{briefing, gc, ingestion};

pub(crate) async fn handle(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    console_error_panic_hook::set_once();
    console_log!("scheduled handler at ts={}", js_sys::Date::now());
    if let Err(e) = ingestion::process_all_feeds(&env).await {
        console_log!("scheduled handler failed: {e}");
    }
    // R2 garbage collection — runs on every cron cycle but is a no-op
    // when there's nothing to expire (no R2 bucket configured, or no
    // articles past the 30-day cutoff with full-text content).
    let now = (js_sys::Date::now() / 1000.0) as i64;
    if let Err(e) = gc::gc_r2_objects(&env, now).await {
        console_log!("gc_r2_objects failed: {e}");
    }
    // Daily Intelligence Brief generation — runs once per day.
    // Uses a KV lock (TTL 1h) to prevent duplicate generation across
    // multiple cron cycles.  Failure is non-fatal (logged, not retried).
    briefing::generate_briefing_task(&env, now).await;
}
