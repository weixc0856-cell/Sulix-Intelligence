//! Provider factory — constructs ModelProvider instances from configuration.
//!
//! The composition root (worker-entry) provides the HTTP client; this module
//! handles the rest: config parsing, provider construction, error types.

use crate::deepseek::{HttpClient, RealDeepSeek};
use crate::provider::ModelProvider;

/// Errors from model runtime initialization.
#[derive(Debug, thiserror::Error)]
pub enum ModelRuntimeError {
    #[error("missing credential: {0}")]
    MissingCredential(String),
    #[error("provider request failed: {0}")]
    ProviderRequestFailed(String),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error("schema parse failed: {0}")]
    SchemaParseFailed(String),
}

/// Configuration for building a model provider.
#[derive(Debug, Clone)]
pub struct ModelRuntimeConfig {
    pub api_key: String,
    pub base_url: String,
    pub chat_model: String,
}

impl ModelRuntimeConfig {
    /// Read configuration from environment variables commonly available in Workers.
    /// `AI_API_KEY`, `AI_BASE_URL`, `AI_CHAT_MODEL`.
    pub fn from_env(
        api_key: &str,
        base_url: Option<&str>,
        chat_model: Option<&str>,
    ) -> Result<Self, ModelRuntimeError> {
        if api_key.is_empty() {
            return Err(ModelRuntimeError::MissingCredential("AI_API_KEY not set".into()));
        }
        Ok(Self {
            api_key: api_key.to_string(),
            base_url: base_url.unwrap_or("https://api.deepseek.com/v1").to_string(),
            chat_model: chat_model.unwrap_or("deepseek-v4-flash").to_string(),
        })
    }
}

/// Build a ModelProvider (RealDeepSeek) from config and an HTTP client.
pub fn build_provider(config: &ModelRuntimeConfig, client: Box<dyn HttpClient>) -> Box<dyn ModelProvider> {
    Box::new(RealDeepSeek::new(config.base_url.clone(), config.api_key.clone(), config.chat_model.clone(), client))
}
