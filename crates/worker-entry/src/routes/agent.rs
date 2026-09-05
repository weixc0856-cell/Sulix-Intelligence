use agent_engine::context::ContextProvider;
use agent_engine::infrastructure::model_provider::ModelProviderLLM;
use agent_engine::runtime::AgentRuntime;
use agent_engine::types::{AgentRequest, ContextResult};
use application::ProductionAppServices;
use context_engine::builder::ContextBuilder;
use infrastructure::context_repository::D1ContextRepository;
use model_runtime::{build_provider, ModelRuntimeConfig};
use store::D1Store;
use worker::*;

use super::response;
use crate::services::http_client::WorkerHttpClient;

struct CtxWrapper(ContextBuilder<D1ContextRepository<D1Store>>);

#[async_trait::async_trait(?Send)]
impl ContextProvider for CtxWrapper {
    async fn build_context(&self, query: &str) -> Result<ContextResult, String> {
        let ctx = self.0.build(query, None, None).await?;
        let confidence = ctx.confidence.overall;
        Ok(ContextResult { snapshot_id: ctx.snapshot_id.clone(), context: ctx, confidence })
    }
}

/// Try to build a model provider from worker environment variables.
fn try_build_provider(env: &Env) -> Option<Box<dyn model_runtime::ModelProvider>> {
    let api_key = env.secret("AI_API_KEY").ok()?;
    let base_url = env.var("AI_BASE_URL").ok().map(|v| v.to_string());
    let chat_model = env.var("AI_CHAT_MODEL").ok().map(|v| v.to_string());
    let config = ModelRuntimeConfig::from_env(&api_key.to_string(), base_url.as_deref(), chat_model.as_deref()).ok()?;
    Some(build_provider(&config, Box::new(WorkerHttpClient)))
}

/// POST /api/internal/agent/run
pub(crate) async fn run(mut req: Request, ctx: RouteContext<ProductionAppServices>) -> Result<Response> {
    let body: AgentRequest = match req.json().await {
        Ok(b) => b,
        Err(_) => return response::json_err(400, "invalid request body"),
    };

    let store = ctx.data.store.clone();

    let provider: Box<dyn model_runtime::ModelProvider> =
        try_build_provider(&ctx.env).unwrap_or_else(|| Box::new(model_runtime::NoopProvider::new()));
    let llm = Box::new(ModelProviderLLM::new(provider));
    let runtime = AgentRuntime::new(Box::new(CtxWrapper(ContextBuilder::new(D1ContextRepository::new(store)))), llm);

    match runtime.execute(body).await {
        Ok(resp) => response::json_ok(serde_json::to_value(resp).unwrap_or_default()),
        Err(e) => {
            console_log!("[agent] execute failed: {e}");
            response::json_err(500, "agent execution failed")
        }
    }
}
