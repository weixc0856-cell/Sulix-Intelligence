//! Decision-reflection route — composition-root owned.
//!
//! POST /api/intelligence/decisions/:id/reflect
//!
//! Migrated from `api` in Phase 2. Reflection orchestrates the reflection
//! engine with adapters assembled from the worker environment (R2, D1
//! repositories, event log, model provider), so the HTTP adapter lives here in
//! worker-entry. Wiring only — `build_engine` / provider construction move
//! wholesale; no domain refactor.

use event_store::{EventR2Backend, NoopEventStore};
use infrastructure::artifact_registry::D1ArtifactRegistry;
use infrastructure::event_log::EventStoreLog;
use infrastructure::reflection_repository::D1ReflectionRepository;
use object_store::R2Store;
use reflection_engine::generator::RealReflectionGenerator;
use reflection_engine::{ReflectionEngine, ReflectionJob, ReflectionTrigger};
use serde_json::json;
use shared_kernel::event_log::EventLog;
use store::{D1Store, Store};
use worker::*;

use super::response;

type ReflectionEngineType = ReflectionEngine<
    D1ReflectionRepository<D1Store>,
    Box<dyn EventLog>,
    RealReflectionGenerator,
    D1ArtifactRegistry<D1Store, R2Store>,
>;

fn build_engine(env: &Env, store: D1Store) -> Result<ReflectionEngineType> {
    let r2_bucket = env.bucket("RAW_CONTENT")?;
    let r2_store = R2Store::new(r2_bucket);
    let event_log: Box<dyn EventLog> = match env.bucket("RAW_CONTENT").ok() {
        Some(_bucket) => Box::new(EventStoreLog::new(Box::new(EventR2Backend::new(
            store.clone(),
            R2Store::new(env.bucket("RAW_CONTENT")?),
        )))),
        _ => Box::new(EventStoreLog::new(Box::new(NoopEventStore::new()))),
    };
    let artifact_registry = D1ArtifactRegistry::new(store.clone(), r2_store);
    let repository = D1ReflectionRepository::new(store);

    let provider = try_build_reflection_provider(env);
    let generator = RealReflectionGenerator::new(provider);
    Ok(ReflectionEngine::new(repository, event_log, generator, artifact_registry))
}

/// Build a model provider for the reflection generator, falling back to NoopProvider.
fn try_build_reflection_provider(env: &Env) -> Box<dyn model_runtime::ModelProvider> {
    if let Ok(api_key) = env.secret("AI_API_KEY") {
        let base_url = env.var("AI_BASE_URL").ok().map(|v| v.to_string());
        let chat_model = env.var("AI_CHAT_MODEL").ok().map(|v| v.to_string());
        if let Ok(config) = model_runtime::ModelRuntimeConfig::from_env(
            &api_key.to_string(),
            base_url.as_deref(),
            chat_model.as_deref(),
        ) {
            let client = WorkerHttpClient;
            return model_runtime::build_provider(&config, Box::new(client));
        }
    }
    Box::new(model_runtime::NoopProvider::new())
}

/// Minimal HTTP client for the reflection route.
struct WorkerHttpClient;

#[async_trait::async_trait(?Send)]
impl model_runtime::HttpClient for WorkerHttpClient {
    async fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, model_runtime::ModelError> {
        let mut init = RequestInit::new();
        init.with_method(Method::Post);
        let wh = worker::Headers::new();
        for (k, v) in headers {
            let _ = wh.set(k, v);
        }
        init.with_headers(wh);
        let body_str =
            serde_json::to_string(body).map_err(|e| model_runtime::ModelError::ProviderError(e.to_string()))?;
        init.with_body(Some(body_str.into()));
        let req = Request::new_with_init(url, &init)
            .map_err(|e| model_runtime::ModelError::ProviderError(format!("{e:?}")))?;
        let mut resp = worker::Fetch::Request(req)
            .send()
            .await
            .map_err(|e| model_runtime::ModelError::ProviderError(format!("{e:?}")))?;
        let status = resp.status_code();
        if status == 429 {
            return Err(model_runtime::ModelError::RateLimited);
        }
        if status == 401 {
            return Err(model_runtime::ModelError::AuthenticationFailed);
        }
        if status >= 500 {
            return Err(model_runtime::ModelError::ProviderError(format!("HTTP {status}")));
        }
        let text = resp.text().await.map_err(|e| model_runtime::ModelError::InvalidResponse(e.to_string()))?;
        serde_json::from_str(&text).map_err(|e| model_runtime::ModelError::InvalidResponse(e.to_string()))
    }
}

/// POST /api/intelligence/decisions/:id/reflect
pub(crate) async fn reflect(_req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let decision_id: i64 = match ctx.param("id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return response::json_err(400, "invalid decision id"),
    };

    let engine = match build_engine(&ctx.env, ctx.data.clone()) {
        Ok(e) => e,
        Err(_) => return response::json_err(503, "service unavailable"),
    };

    let now = (js_sys::Date::now() / 1000.0) as i64;
    let job_id = format!("job_reflect_DEC{decision_id:06}_{now}");

    let job = ReflectionJob { decision_id, trigger: ReflectionTrigger::Api, correlation_id: job_id.clone() };

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
