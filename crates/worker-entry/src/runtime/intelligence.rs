//! IntelligenceRuntime — shared model provider for the intelligence pipeline.

use model_runtime::{build_provider, ModelProvider, ModelRuntimeConfig, ModelRuntimeError};
use worker::*;

use crate::services::http_client::WorkerHttpClient;

/// Shared intelligence runtime — holds the model provider.
pub struct IntelligenceRuntime {
    pub provider: Box<dyn ModelProvider>,
}

impl IntelligenceRuntime {
    /// Create a new IntelligenceRuntime from the Worker environment.
    pub fn new(env: &Env) -> Result<Self, ModelRuntimeError> {
        let api_key = env.secret("AI_API_KEY").map(|v| v.to_string()).unwrap_or_default();
        let base_url = env.var("AI_BASE_URL").ok().map(|v| v.to_string());
        let chat_model = env.var("AI_CHAT_MODEL").ok().map(|v| v.to_string());
        let config = ModelRuntimeConfig::from_env(&api_key, base_url.as_deref(), chat_model.as_deref())?;
        let client = Box::new(WorkerHttpClient);
        let provider = build_provider(&config, client);
        Ok(Self { provider })
    }
}
