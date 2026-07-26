//! ModelProvider trait — the single interface for all LLM interactions.
//!
//! Every intelligence task (summarization, claim extraction, reflection,
//! agent answer) goes through this trait. Providers are interchangeable:
//! `RealDeepSeek` for production, `NoopProvider` for tests.

use async_trait::async_trait;

use crate::types::{ModelCapabilities, ModelError, ModelRequest, ModelResponse};

/// Unified model provider trait.
///
/// Implementations:
/// - `RealDeepSeek` — production, calls DeepSeek API
/// - `NoopProvider` — tests, returns deterministic responses
///
#[async_trait(?Send)]
pub trait ModelProvider {
    /// Return the capabilities of this provider.
    fn capabilities(&self) -> ModelCapabilities;

    /// Generate a response for the given request.
    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse, ModelError>;
}
