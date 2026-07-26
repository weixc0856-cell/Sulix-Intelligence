//! NoopProvider — deterministic stub for testing.
//!
//! Returns a fixed JSON response for any input. Does NOT call any external API.

use async_trait::async_trait;

use crate::provider::ModelProvider;
use crate::types::{ModelCapabilities, ModelError, ModelRequest, ModelResponse};

/// A model provider that returns deterministic responses without calling any API.
/// Useful for tests and development without network access.
pub struct NoopProvider {
    /// Fixed response text to return.
    response_text: String,
    /// Whether to return a parsed JSON value.
    response_json: Option<serde_json::Value>,
}

impl NoopProvider {
    pub fn new() -> Self {
        Self { response_text: "Noop response — model-runtime not configured.".into(), response_json: None }
    }

    /// Create a provider that returns the given text.
    pub fn with_text(text: &str) -> Self {
        Self { response_text: text.into(), response_json: None }
    }

    /// Create a provider that returns the given JSON.
    pub fn with_json(json: serde_json::Value) -> Self {
        Self { response_text: json.to_string(), response_json: Some(json) }
    }
}

impl Default for NoopProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait(?Send)]
impl ModelProvider for NoopProvider {
    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            provider: "noop".into(),
            model_name: "noop".into(),
            context_window: 4096,
            supports_json: false,
        }
    }

    async fn generate(&self, _request: ModelRequest) -> Result<ModelResponse, ModelError> {
        Ok(ModelResponse {
            text: self.response_text.clone(),
            parsed: self.response_json.clone(),
            usage: None,
            finish_reason: "stop".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ModelRequest;
    use crate::types::{GenerationParams, ModelTask};

    #[test]
    fn noop_returns_fixed_text() {
        let provider = NoopProvider::with_text("test response");
        let request = ModelRequest {
            task: ModelTask::AgentAnswer,
            system_prompt: "test".into(),
            context: vec![],
            output_schema: None,
            parameters: GenerationParams::default(),
        };
        let result = futures::executor::block_on(provider.generate(request)).unwrap();
        assert_eq!(result.text, "test response");
    }

    #[test]
    fn noop_returns_json() {
        let json = serde_json::json!({"key": "value"});
        let provider = NoopProvider::with_json(json.clone());
        let request = ModelRequest {
            task: ModelTask::Summarization,
            system_prompt: "test".into(),
            context: vec![],
            output_schema: None,
            parameters: GenerationParams::default(),
        };
        let result = futures::executor::block_on(provider.generate(request)).unwrap();
        assert_eq!(result.parsed, Some(json));
    }
}
