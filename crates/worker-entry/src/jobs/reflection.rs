use event_store::{EventR2Backend, EventStore, NoopEventStore};
use object_store::R2Store;
use reflection_engine::generator::RealReflectionGenerator;
use reflection_engine::{ReflectionEngine, ReflectionJob, ReflectionTrigger};
use store::D1Store;
use worker::*;

use crate::runtime::intelligence::IntelligenceRuntime;

const MAX_PER_CYCLE: u32 = 3;

pub(crate) async fn process_pending_reflections(env: &Env, now: i64) {
    let store = match env.d1("DB") {
        Ok(db) => D1Store::new(db),
        Err(e) => {
            console_log!("[reflection] D1 binding failed: {e}");
            return;
        }
    };

    let event_store: Box<dyn EventStore> = match (env.d1("DB").ok(), env.bucket("RAW_CONTENT").ok()) {
        (Some(db), Some(bucket)) => Box::new(EventR2Backend::new(D1Store::new(db), R2Store::new(bucket))),
        _ => Box::new(NoopEventStore::new()),
    };

    let provider = IntelligenceRuntime::new(env)
        .map(|r| r.provider)
        .unwrap_or_else(|_| Box::new(model_runtime::NoopProvider::new()));
    let generator = RealReflectionGenerator::new(provider);
    let engine = ReflectionEngine::new(store, event_store, generator);

    // 1. Stale recovery
    if let Ok(stale) = engine.repository().stale_generating_reflections(now).await {
        for r in &stale {
            let _ = engine
                .repository()
                .update_reflection(&store::UpdateReflection {
                    id: r.id,
                    status: "failed".into(),
                    result: None,
                    quality_score: None,
                    artifact_key: None,
                    lessons_count: None,
                    rules_count: None,
                    retry_count: Some(r.retry_count + 1),
                    last_error: Some("lease_expired".into()),
                    started_at: None,
                    lease_until: None,
                })
                .await;
            console_log!("[reflection] stale recovery: REF-{:06} -> failed", r.id);
        }
    }

    // 2. New eligible decisions
    let eligible = engine.repository().decisions_eligible_for_reflection(now, MAX_PER_CYCLE).await.unwrap_or_default();

    // 3. Failed retries
    let failed = engine.repository().failed_reflections_for_retry(MAX_PER_CYCLE).await.unwrap_or_default();

    let mut to_process: Vec<i64> = eligible;
    for r in &failed {
        if to_process.len() >= MAX_PER_CYCLE as usize {
            break;
        }
        to_process.push(r.decision_id);
    }

    for decision_id in to_process {
        let correlation_id = format!("cron_reflect_DEC{decision_id:06}_{now}");
        let job = ReflectionJob { decision_id, trigger: ReflectionTrigger::Cron, correlation_id };
        match engine.execute(&job).await {
            Ok(r) => console_log!("[reflection] REF-{:06} generated for DEC-{:06}", r.reflection_id, r.decision_id),
            Err(e) => console_log!("[reflection] DEC-{:06} failed: {e}", decision_id),
        }
    }
}
