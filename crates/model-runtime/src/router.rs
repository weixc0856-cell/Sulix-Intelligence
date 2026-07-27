//! ProviderRouter — capability-based model routing.
//!
//! Routes each request to the appropriate provider based on task requirements
//! and provider capabilities. Supports cost-aware selection.

use std::sync::Arc;

use async_trait::async_trait;

use crate::provider::ModelProvider;
use crate::types::{ModelError, ModelRequest, ModelResponse, ModelTask};

/// Model capabilities — used for routing decisions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Fast, low-latency generation.
    FastGeneration,
    /// Complex multi-step reasoning.
    Reasoning,
    /// Large context window (>32K tokens).
    LongContext,
    /// JSON structured output.
    StructuredOutput,
    /// Low cost per token.
    LowCost,
}

/// A registered provider with its capabilities.
struct ProviderEntry {
    provider: Arc<dyn ModelProvider>,
    capabilities: Vec<Capability>,
    #[allow(dead_code)]
    priority: u32, // lower = preferred
}

/// Routes requests to the appropriate model provider based on capabilities.
pub struct ProviderRouter {
    entries: Vec<ProviderEntry>,
}

impl ProviderRouter {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Register a provider with its capabilities.
    pub fn register(&mut self, provider: Arc<dyn ModelProvider>, capabilities: Vec<Capability>, priority: u32) {
        self.entries.push(ProviderEntry { provider, capabilities, priority });
    }
}

impl Default for ProviderRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Map a ModelTask to the required capabilities.
fn task_capabilities(task: ModelTask) -> Vec<Capability> {
    match task {
        ModelTask::Summarization => vec![Capability::FastGeneration, Capability::StructuredOutput],
        ModelTask::ClaimExtraction => vec![Capability::Reasoning, Capability::StructuredOutput],
        ModelTask::Reflection => vec![Capability::Reasoning, Capability::LongContext],
        ModelTask::AgentAnswer => vec![Capability::Reasoning],
    }
}

/// Check if a provider's capabilities satisfy all requirements.
fn satisfies(provider_caps: &[Capability], required: &[Capability]) -> bool {
    required.iter().all(|r| provider_caps.contains(r))
}

#[async_trait(?Send)]
impl ModelProvider for ProviderRouter {
    fn capabilities(&self) -> crate::types::ModelCapabilities {
        self.entries.first().map(|e| e.provider.capabilities()).unwrap_or_else(|| crate::types::ModelCapabilities {
            provider: "router".into(),
            model_name: "router".into(),
            context_window: 0,
            supports_json: false,
        })
    }

    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse, ModelError> {
        let required = task_capabilities(request.task);

        // Find the first (highest priority) provider satisfying all requirements
        for entry in &self.entries {
            if satisfies(&entry.capabilities, &required) {
                return entry.provider.generate(request).await;
            }
        }

        // Fallback: try the first available provider
        self.entries
            .first()
            .ok_or_else(|| ModelError::ProviderError("No model providers registered".into()))?
            .provider
            .generate(request)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{GenerationParams, ModelCapabilities, ModelRequest, ModelResponse, ModelTask, TokenUsage};

    struct MockProvider {
        name: String,
    }

    #[async_trait(?Send)]
    impl ModelProvider for MockProvider {
        fn capabilities(&self) -> ModelCapabilities {
            ModelCapabilities {
                provider: self.name.clone(),
                model_name: self.name.clone(),
                context_window: 4096,
                supports_json: true,
            }
        }
        async fn generate(&self, _request: ModelRequest) -> Result<ModelResponse, ModelError> {
            Ok(ModelResponse {
                text: format!("response from {}", self.name),
                parsed: None,
                usage: Some(TokenUsage { prompt_tokens: 10, completion_tokens: 10 }),
                finish_reason: "stop".into(),
            })
        }
    }

    #[test]
    fn routes_by_capability() {
        let mut router = ProviderRouter::new();
        router.register(
            Arc::new(MockProvider { name: "fast".into() }),
            vec![Capability::FastGeneration, Capability::LowCost],
            1,
        );
        router.register(
            Arc::new(MockProvider { name: "smart".into() }),
            vec![Capability::Reasoning, Capability::StructuredOutput],
            2,
        );

        // Fast task → fast provider
        let req = ModelRequest {
            task: ModelTask::Summarization,
            system_prompt: "test".into(),
            context: vec![],
            output_schema: None,
            parameters: GenerationParams::default(),
        };
        let result = futures::executor::block_on(router.generate(req)).unwrap();
        assert_eq!(result.text, "response from fast");

        // Reasoning task → smart provider
        let req = ModelRequest {
            task: ModelTask::ClaimExtraction,
            system_prompt: "test".into(),
            context: vec![],
            output_schema: None,
            parameters: GenerationParams::default(),
        };
        let result = futures::executor::block_on(router.generate(req)).unwrap();
        assert_eq!(result.text, "response from smart");
    }
}
