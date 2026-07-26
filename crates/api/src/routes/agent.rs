use crate::shared::response;
use agent_engine::context::ContextProvider;
use agent_engine::infrastructure::model_provider::ModelProviderLLM;
use agent_engine::runtime::AgentRuntime;
use agent_engine::types::{AgentRequest, ContextResult};
use context_engine::builder::ContextBuilder;
use model_runtime::{build_provider, ModelRuntimeConfig};
use store::D1Store;
use worker::*;

struct CtxWrapper(ContextBuilder<D1Store>);

#[async_trait::async_trait(?Send)]
impl ContextProvider for CtxWrapper {
    async fn build_context(&self, query: &str) -> Result<ContextResult, String> {
        let ctx = self.0.build(query, None, None, None).await?;
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

/// Minimal HTTP client implementation using worker::Fetch for model runtime.
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
            .map_err(|e| model_runtime::ModelError::ProviderError(format!("request creation: {e:?}")))?;
        let mut resp = worker::Fetch::Request(req)
            .send()
            .await
            .map_err(|e| model_runtime::ModelError::ProviderError(format!("fetch: {e:?}")))?;

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

/// POST /api/internal/agent/run
pub async fn run(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let body: AgentRequest = match req.json().await {
        Ok(b) => b,
        Err(_) => return response::json_err(400, "invalid request body"),
    };

    let store = match ctx.env.d1("DB") {
        Ok(db) => D1Store::new(db),
        Err(e) => {
            console_log!("[agent] D1 binding failed: {e}");
            return response::json_err(503, "service unavailable");
        }
    };

    let provider: Box<dyn model_runtime::ModelProvider> =
        try_build_provider(&ctx.env).unwrap_or_else(|| Box::new(model_runtime::NoopProvider::new()));
    let llm = Box::new(ModelProviderLLM::new(provider));
    let runtime = AgentRuntime::new(Box::new(CtxWrapper(ContextBuilder::new(store))), llm);

    match runtime.execute(body).await {
        Ok(response) => response::json_ok(serde_json::to_value(response).unwrap_or_default()),
        Err(e) => {
            console_log!("[agent] execute failed: {e}");
            response::json_err(500, "agent execution failed")
        }
    }
}
