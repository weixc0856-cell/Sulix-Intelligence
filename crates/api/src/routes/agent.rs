use crate::shared::response;
use agent_engine::context::ContextProvider;
use agent_engine::llm::noop::NoopLLM;
use agent_engine::runtime::AgentRuntime;
use agent_engine::types::{AgentRequest, ContextResult};
use context_engine::builder::ContextBuilder;
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

    let runtime = AgentRuntime::new(Box::new(CtxWrapper(ContextBuilder::new(store))), Box::new(NoopLLM));

    match runtime.execute(body).await {
        Ok(response) => response::json_ok(serde_json::to_value(response).unwrap_or_default()),
        Err(e) => {
            console_log!("[agent] execute failed: {e}");
            response::json_err(500, "agent execution failed")
        }
    }
}
