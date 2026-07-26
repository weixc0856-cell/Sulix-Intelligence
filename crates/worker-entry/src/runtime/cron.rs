use worker::*;

use crate::jobs::{archive, briefing, gc, ingestion, memory, reflection, signal};

/// Feature flags for cron jobs. All default to false (disabled).
/// Enable individually via env vars (e.g. CRON_INGESTION_ENABLED=true).
struct CronConfig {
    pub ingestion_enabled: bool,
    pub signal_enabled: bool,
    pub reflection_enabled: bool,
    pub memory_enabled: bool,
}

impl CronConfig {
    fn from_env(env: &Env) -> Self {
        let v = |key: &str| -> bool {
            env.var(key).ok().and_then(|v| v.to_string().parse().ok()).unwrap_or(false)
        };
        Self {
            ingestion_enabled: v("CRON_INGESTION_ENABLED"),
            signal_enabled: v("CRON_SIGNAL_ENABLED"),
            reflection_enabled: v("CRON_REFLECTION_ENABLED"),
            memory_enabled: v("CRON_MEMORY_ENABLED"),
        }
    }
}

pub(crate) async fn handle(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    console_error_panic_hook::set_once();
    let cfg = CronConfig::from_env(&env);
    console_log!("scheduled handler at ts={}", js_sys::Date::now());

    // Feed ingestion (feature-flagged for D1 quota recovery)
    if cfg.ingestion_enabled {
        if let Err(e) = ingestion::process_all_feeds(&env).await {
            console_log!("scheduled handler: feed ingestion failed: {e}");
        }
    }

    // R2 garbage collection
    let now = (js_sys::Date::now() / 1000.0) as i64;
    if let Err(e) = gc::gc_r2_objects(&env, now).await {
        console_log!("gc_r2_objects failed: {e}");
    }

    // Signal Engine (feature-flagged)
    if cfg.signal_enabled {
        signal::run_signal_engine(&env, now).await;
    }

    // Daily Intelligence Brief generation — runs once per day (KV lock).
    briefing::generate_briefing_task(&env, now).await;

    // Object Outbox — drain pending archive entries to the R2 Memory Archive.
    // Always runs (essential for outbox drain). Batch limited internally.
    archive::archive_outbox(&env).await;

    // Decision Reflection (feature-flagged)
    if cfg.reflection_enabled {
        reflection::process_pending_reflections(&env, now).await;
    }

    // Memory Engine (feature-flagged)
    if cfg.memory_enabled {
        memory::process_pending(&env, now).await;
    }
}
