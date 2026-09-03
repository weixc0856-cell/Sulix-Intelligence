use event_store::{EventR2Backend, NoopEventStore};
use infrastructure::artifact_registry::D1ArtifactRegistry;
use infrastructure::event_log::EventStoreLog;
use infrastructure::reflection_repository::D1ReflectionRepository;
use object_store::R2Store;
use reflection_engine::generator::RealReflectionGenerator;
use reflection_engine::{ReflectionEngine, ReflectionJob, ReflectionTrigger};
use shared_kernel::event_log::EventLog;
use store::{D1Store, UpdateReflection};
use worker::*;

use crate::runtime::intelligence::IntelligenceRuntime;

type ReflectionEngineType = ReflectionEngine<
    D1ReflectionRepository<D1Store>,
    Box<dyn EventLog>,
    RealReflectionGenerator,
    D1ArtifactRegistry<D1Store, R2Store>,
>;

const MAX_PER_CYCLE: u32 = 3;

fn build_engine(env: &Env) -> Result<ReflectionEngineType, String> {
    let r2_bucket = env.bucket("RAW_CONTENT").map_err(|e| format!("R2 binding: {e}"))?;
    let r2_store = R2Store::new(r2_bucket);
    let event_log: Box<dyn EventLog> = match (env.d1("DB").ok(), env.bucket("RAW_CONTENT").ok()) {
        (Some(db), Some(_bucket)) => Box::new(EventStoreLog::new(Box::new(EventR2Backend::new(
            D1Store::new(db),
            R2Store::new(env.bucket("RAW_CONTENT").map_err(|e| format!("R2 binding 2: {e}"))?),
        )))),
        _ => Box::new(EventStoreLog::new(Box::new(NoopEventStore::new()))),
    };
    let artifact_registry =
        D1ArtifactRegistry::new(D1Store::new(env.d1("DB").map_err(|e| format!("D1 binding: {e}"))?), r2_store);
    let repository = D1ReflectionRepository::new(D1Store::new(env.d1("DB").map_err(|e| format!("D1 binding 2: {e}"))?));

    let provider = IntelligenceRuntime::new(env)
        .map(|r| r.provider)
        .unwrap_or_else(|_| Box::new(model_runtime::NoopProvider::new()));
    let generator = RealReflectionGenerator::new(provider);

    Ok(ReflectionEngine::new(repository, event_log, generator, artifact_registry))
}

pub(crate) async fn process_pending_reflections(env: &Env, now: i64) {
    let engine = match build_engine(env) {
        Ok(e) => e,
        Err(e) => {
            console_log!("[reflection] engine build failed: {e}");
            return;
        }
    };

    // Scheduling queries live on the composition root (worker-entry), which may
    // depend on store; the engine itself only sees its ReflectionRepository.
    let sched = match env.d1("DB") {
        Ok(db) => D1Store::new(db),
        Err(e) => {
            console_log!("[reflection] D1 binding failed: {e}");
            return;
        }
    };

    // 1. Stale recovery
    if let Ok(stale) = sched.stale_generating_reflections(now).await {
        for r in &stale {
            let _ = sched
                .update_reflection(&UpdateReflection {
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
    let eligible = sched.decisions_eligible_for_reflection(now, MAX_PER_CYCLE).await.unwrap_or_default();

    // 3. Failed retries
    let failed = sched.failed_reflections_for_retry(MAX_PER_CYCLE).await.unwrap_or_default();

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
