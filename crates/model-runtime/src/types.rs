//! Request/response types for the ModelProvider interface.
//!
//! These types define the contract between Sulix's intelligence pipeline
//! and any underlying LLM provider. The `ModelTask` enum allows the provider
//! to apply task-specific optimizations (token budgets, system prompts, etc.).

use serde::{Deserialize, Serialize};

/// Identifies what kind of reasoning task this request is for.
/// Used for observability (Trust Dashboard) and provider-specific tuning.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ModelTask {
    Summarization,
    ClaimExtraction,
    Reflection,
    AgentAnswer,
}

impl std::fmt::Display for ModelTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Summarization => write!(f, "summarization"),
            Self::ClaimExtraction => write!(f, "claim_extraction"),
            Self::Reflection => write!(f, "reflection"),
            Self::AgentAnswer => write!(f, "agent"),
        }
    }
}

/// A structured context block for the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBlock {
    pub title: String,
    pub content: String,
    /// Priority for token budget allocation (0.0–1.0).
    pub priority: f64,
}

/// Generation parameters for a model request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationParams {
    pub temperature: f64,
    pub max_tokens: u32,
}

impl Default for GenerationParams {
    fn default() -> Self {
        Self { temperature: 0.3, max_tokens: 2048 }
    }
}

/// A request to a model provider.
#[derive(Debug, Clone)]
pub struct ModelRequest {
    /// Identifies what kind of reasoning task this is.
    pub task: ModelTask,
    /// The system prompt guiding model behaviour.
    pub system_prompt: String,
    /// Structured context blocks (evidence, history, etc.).
    pub context: Vec<ContextBlock>,
    /// Optional JSON schema for structured output parsing.
    pub output_schema: Option<serde_json::Value>,
    /// Generation parameters.
    pub parameters: GenerationParams,
}

/// Token usage statistics returned by the model provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

/// Response from a model provider.
#[derive(Debug, Clone)]
pub struct ModelResponse {
    /// Raw text output from the model.
    pub text: String,
    /// Parsed JSON output (populated when output_schema was provided).
    pub parsed: Option<serde_json::Value>,
    /// Token usage statistics.
    pub usage: Option<TokenUsage>,
    /// Reason the generation finished.
    pub finish_reason: String,
}

/// Model capabilities metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub provider: String,
    pub model_name: String,
    pub context_window: u32,
    pub supports_json: bool,
}

/// Errors from model provider operations.
#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("authentication failed")]
    AuthenticationFailed,
    #[error("rate limited")]
    RateLimited,
    #[error("request timed out")]
    Timeout,
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error("provider error: {0}")]
    ProviderError(String),
}
