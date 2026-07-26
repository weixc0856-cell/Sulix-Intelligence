use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct ModelCapability {
    pub provider: String,
    pub model_name: String,
    pub context_window: u32,
    pub supports_json: bool,
}

#[derive(Debug, Clone)]
pub struct LLMRequest {
    pub system_prompt: String,
    pub user_message: String,
    pub max_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct LLMResponse {
    pub text: String,
    pub finish_reason: String,
    pub usage: Option<LLMUsage>,
}

#[derive(Debug, Clone)]
pub struct LLMUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

#[derive(Debug)]
pub enum LLMError {
    AuthenticationFailed,
    RateLimited,
    Timeout,
    InvalidResponse,
    ProviderError(String),
}

#[async_trait(?Send)]
pub trait LLMProvider {
    fn capability(&self) -> ModelCapability;
    async fn complete(&self, request: LLMRequest) -> Result<LLMResponse, LLMError>;
}
