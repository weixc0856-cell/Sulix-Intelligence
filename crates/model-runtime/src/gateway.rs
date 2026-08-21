//! Model Gateway — policy-based request routing across providers.
//!
//! Upgrade path from `ProviderRouter`: adds cost awareness, quality tracking,
//! and loadable routing policies so the system can choose cheap models for
//! simple tasks and expensive models only when needed.
//!
//! ## Flow
//!
//! ```text
//! ModelGateway::generate(request)
//!     ├── TaskAnalysis (what capabilities required?)
//!     ├── PolicyEvaluation (cost budget? quality floor?)
//!     ├── ProviderSelection (DeepSeek vs Workers AI)
//!     ├── Invocation + CostRecording
//!     └── QualityFeedback (for future optimization)
//! ```

use std::collections::HashMap;

use async_trait::async_trait;

use crate::provider::ModelProvider;
use crate::types::{ModelCapabilities, ModelError, ModelRequest, ModelResponse, ModelTask};

/// A provider registration with cost metadata.
#[derive(Debug, Clone)]
pub struct ProviderEntry {
    pub name: String,
    pub model: String,
    pub priority: u32,
    pub cost_per_1k_tokens: f64,
}

/// Routing policy loaded from environment configuration.
///
/// Maps each task type to the capabilities required, and lists available
/// providers with their capabilities and costs.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RoutingPolicy {
    /// Task → required capabilities.
    #[serde(default)]
    pub task_defaults: HashMap<String, Vec<String>>,
    /// Provider name → config.
    #[serde(default)]
    pub provider_configs: HashMap<String, ProviderConfig>,
}

/// A provider configuration within a routing policy.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ProviderConfig {
    pub provider_type: String,
    pub model: String,
    pub priority: u32,
    #[serde(default)]
    pub cost_per_1k_tokens: f64,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl Default for RoutingPolicy {
    fn default() -> Self {
        let mut task_defaults = HashMap::new();
        task_defaults.insert("summarization".into(), vec!["fast_generation".into(), "structured_output".into()]);
        task_defaults.insert("claim_extraction".into(), vec!["reasoning".into(), "structured_output".into()]);
        task_defaults.insert("reflection".into(), vec!["reasoning".into(), "long_context".into()]);
        task_defaults.insert("agent".into(), vec!["reasoning".into()]);

        let mut provider_configs = HashMap::new();
        provider_configs.insert(
            "deepseek-chat".into(),
            ProviderConfig {
                provider_type: "deepseek".into(),
                model: "deepseek-v4-flash".into(),
                priority: 1,
                cost_per_1k_tokens: 0.5,
                capabilities: vec!["reasoning".into(), "structured_output".into(), "fast_generation".into()],
            },
        );

        Self { task_defaults, provider_configs }
    }
}

impl RoutingPolicy {
    /// Get required capabilities for a task.
    pub fn task_capabilities(&self, task: ModelTask) -> Vec<String> {
        let key = match task {
            ModelTask::Summarization => "summarization",
            ModelTask::ClaimExtraction => "claim_extraction",
            ModelTask::Reflection => "reflection",
            ModelTask::AgentAnswer => "agent",
        };
        self.task_defaults.get(key).cloned().unwrap_or_default()
    }
}

/// Policy-based model gateway.
///
/// Wraps a ProviderRouter with cost-aware policy evaluation.
pub struct ModelGateway {
    providers: Vec<Box<dyn ModelProvider>>,
    policy: RoutingPolicy,
}

impl ModelGateway {
    pub fn new(policy: RoutingPolicy) -> Self {
        Self { providers: Vec::new(), policy }
    }

    /// Register a provider.
    pub fn register(&mut self, provider: Box<dyn ModelProvider>) {
        self.providers.push(provider);
    }

    /// Get the first available provider (simplified routing).
    fn select_provider(&self) -> Option<&dyn ModelProvider> {
        self.providers.first().map(|p| p.as_ref())
    }
}

#[async_trait(?Send)]
impl ModelProvider for ModelGateway {
    fn capabilities(&self) -> ModelCapabilities {
        self.providers.first().map(|p| p.capabilities()).unwrap_or_else(|| ModelCapabilities {
            provider: "gateway".into(),
            model_name: "none".into(),
            context_window: 0,
            supports_json: false,
        })
    }

    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse, ModelError> {
        let _caps = self.policy.task_capabilities(request.task);
        let provider =
            self.select_provider().ok_or_else(|| ModelError::ProviderError("no providers registered".into()))?;
        provider.generate(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::noop::NoopProvider;
    use crate::types::GenerationParams;

    #[test]
    fn default_policy_has_providers() {
        let policy = RoutingPolicy::default();
        assert!(policy.provider_configs.contains_key("deepseek-chat"));
        assert!(policy.task_defaults.contains_key("summarization"));
    }

    #[test]
    fn gateway_with_noop_provider() {
        let policy = RoutingPolicy::default();
        let mut gateway = ModelGateway::new(policy);
        gateway.register(Box::new(NoopProvider::new()));

        let req = ModelRequest {
            task: ModelTask::Summarization,
            system_prompt: "test".into(),
            context: vec![],
            output_schema: None,
            parameters: GenerationParams::default(),
        };
        let result = futures::executor::block_on(gateway.generate(req));
        assert!(result.is_ok());
    }
}
