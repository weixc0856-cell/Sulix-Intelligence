use reflection_engine::{ReflectionEngine, ReflectionJob, ReflectionTrigger};
use reflection_engine::generator::{ReflectionGenerator, ReflectionDraft, LessonDraft};
use reflection_engine::context::ReflectionContext;
use serde_json::json;
use store::D1Store;
use event_store::{EventR2Backend, EventStore, NoopEventStore};
use object_store::R2Store;
use worker::*;
use crate::shared::response;

struct NoopGenerator;

#[async_trait::async_trait(?Send)]
impl ReflectionGenerator for NoopGenerator {
    async fn generate(&self, _context: &ReflectionContext) -> Result<ReflectionDraft, String> {
        Ok(ReflectionDraft {
            result: "mixed".into(),
            confidence_calibration: "accurate".into(),
            quality_score: 0.7,
            lessons: vec![LessonDraft {
                category: "general".into(), domain: "default".into(),
                description: "This is a placeholder reflection until LLM integration is connected.".into(),
                severity: "medium".into(), confidence: 0.7, evidence_basis: vec!["PLACEHOLDER".into()],
            }],
            rules: vec![],
        })
    }
}

fn build_engine(env: &Env) -> Result<ReflectionEngine<D1Store, Box<dyn EventStore>, NoopGenerator>> {
    let store = D1Store::new(env.d1("DB")?);
    let event_store: Box<dyn EventStore> = match (env.d1("DB").ok(), env.bucket("RAW_CONTENT").ok()) {
        (Some(db), Some(bucket)) => Box::new(EventR2Backend::new(D1Store::new(db), R2Store::new(bucket))),
        _ => Box::new(NoopEventStore::new()),
    };
    Ok(ReflectionEngine::new(store, event_store, NoopGenerator))
}

/// POST /api/intelligence/decisions/:id/reflect
pub async fn reflect(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let decision_id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid decision id"),
    };

    let engine = match build_engine(&ctx.env) {
        Ok(e) => e,
        Err(_) => return response::json_err(503, "service unavailable"),
    };

    let now = (js_sys::Date::now() / 1000.0) as i64;
    let job_id = format!("job_reflect_DEC{decision_id:06}_{now}");

    let job = ReflectionJob {
        decision_id,
        trigger: ReflectionTrigger::Api,
        correlation_id: job_id.clone(),
    };

    match engine.execute(&job).await {
        Ok(result) => response::json_ok(json!({
            "success": true,
            "reflection_id": format!("REF-{:06}", result.reflection_id),
            "decision_id": format!("DEC-{:06}", decision_id),
            "status": result.status,
        })),
        Err(e) => response::json_err(502, &format!("reflection failed: {e}")),
    }
}
