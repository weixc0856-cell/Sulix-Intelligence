//! ModelProviderLLM — adapts model_runtime::ModelProvider to agent_engine::LLMProvider.

use async_trait::async_trait;

use crate::llm::provider::{LLMError, LLMProvider, LLMRequest, LLMResponse, LLMUsage, ModelCapability};

/// Wraps a `Box<dyn ModelProvider>` as an `LLMProvider` for the agent engine.
pub struct ModelProviderLLM {
    provider: Box<dyn model_runtime::ModelProvider>,
}

impl ModelProviderLLM {
    pub fn new(provider: Box<dyn model_runtime::ModelProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait(?Send)]
impl LLMProvider for ModelProviderLLM {
    fn capability(&self) -> ModelCapability {
        let caps = self.provider.capabilities();
        ModelCapability {
            provider: caps.provider,
            model_name: caps.model_name,
            context_window: caps.context_window,
            supports_json: caps.supports_json,
        }
    }

    async fn complete(&self, request: LLMRequest) -> Result<LLMResponse, LLMError> {
        let model_request = model_runtime::ModelRequest {
            task: model_runtime::ModelTask::AgentAnswer,
            system_prompt: request.system_prompt,
            context: vec![model_runtime::ContextBlock {
                title: "user".into(),
                content: request.user_message,
                priority: 1.0,
            }],
            output_schema: None,
            parameters: model_runtime::GenerationParams { temperature: 0.3, max_tokens: request.max_tokens },
        };

        let response = self.provider.generate(model_request).await.map_err(|e| match e {
            model_runtime::ModelError::AuthenticationFailed => LLMError::AuthenticationFailed,
            model_runtime::ModelError::RateLimited => LLMError::RateLimited,
            model_runtime::ModelError::Timeout => LLMError::Timeout,
            model_runtime::ModelError::InvalidResponse(_) => LLMError::InvalidResponse,
            model_runtime::ModelError::ProviderError(msg) => LLMError::ProviderError(msg),
        })?;

        Ok(LLMResponse {
            text: response.text,
            finish_reason: response.finish_reason,
            usage: response
                .usage
                .map(|u| LLMUsage { prompt_tokens: u.prompt_tokens, completion_tokens: u.completion_tokens }),
        })
    }
}
