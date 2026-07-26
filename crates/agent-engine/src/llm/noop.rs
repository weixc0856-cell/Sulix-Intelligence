use crate::llm::provider::{LLMError, LLMProvider, LLMRequest, LLMResponse, ModelCapability};

use async_trait::async_trait;

pub struct NoopLLM;

#[async_trait(?Send)]
impl LLMProvider for NoopLLM {
    fn capability(&self) -> ModelCapability {
        ModelCapability {
            provider: "noop".into(),
            model_name: "noop".into(),
            context_window: 4096,
            supports_json: false,
        }
    }

    async fn complete(&self, _request: LLMRequest) -> Result<LLMResponse, LLMError> {
        Ok(LLMResponse {
            text: "Noop response — LLM not configured.".into(),
            finish_reason: "stop".into(),
            usage: None,
        })
    }
}
