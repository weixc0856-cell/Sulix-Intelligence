//! RealDeepSeek — production model provider calling DeepSeek API.
//!
//! HTTP transport is abstracted behind [`HttpClient`] so this crate does not
//! depend on `worker::Fetch` — the composition root (`worker-entry`) provides
//! a worker-based implementation. Tests can provide a mock client.

use async_trait::async_trait;

use crate::provider::ModelProvider;
use crate::types::{ModelCapabilities, ModelError, ModelRequest, ModelResponse, TokenUsage};

/// Minimal HTTP client abstraction for model API calls.
#[async_trait(?Send)]
pub trait HttpClient {
    /// POST JSON to a URL with headers, return parsed JSON response.
    async fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ModelError>;
}

/// Production model provider that calls the DeepSeek API.
pub struct RealDeepSeek {
    base_url: String,
    api_key: String,
    chat_model: String,
    client: Box<dyn HttpClient>,
}

impl RealDeepSeek {
    /// Create a new RealDeepSeek provider.
    pub fn new(base_url: String, api_key: String, chat_model: String, client: Box<dyn HttpClient>) -> Self {
        Self { base_url, api_key, chat_model, client }
    }

    fn auth_headers(&self) -> Vec<(String, String)> {
        vec![
            ("Content-Type".into(), "application/json".into()),
            ("Authorization".into(), format!("Bearer {}", self.api_key)),
        ]
    }

    async fn post_json(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value, ModelError> {
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), path);
        self.client.post_json(&url, &self.auth_headers(), body).await
    }
}

#[async_trait(?Send)]
impl ModelProvider for RealDeepSeek {
    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            provider: "deepseek".into(),
            model_name: self.chat_model.clone(),
            context_window: 65536,
            supports_json: true,
        }
    }

    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse, ModelError> {
        let user_content = if request.context.is_empty() {
            request.system_prompt.clone()
        } else {
            let ctx = request
                .context
                .iter()
                .map(|b| format!("## {}\n{}", b.title, b.content))
                .collect::<Vec<_>>()
                .join("\n\n");
            format!("{}\n\n{}", request.system_prompt, ctx)
        };

        let mut body = serde_json::json!({
            "model": self.chat_model,
            "messages": [
                {"role": "user", "content": user_content}
            ],
            "max_tokens": request.parameters.max_tokens,
            "temperature": request.parameters.temperature,
        });

        if request.output_schema.is_some() {
            body["response_format"] = serde_json::json!({"type": "json_object"});
        }

        let json = self.post_json("/chat/completions", &body).await?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| ModelError::InvalidResponse("missing message content".into()))?
            .to_string();

        let parsed = request.output_schema.as_ref().and_then(|_| serde_json::from_str(&content).ok());

        let usage = json["usage"].as_object().map(|u| TokenUsage {
            prompt_tokens: u["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: u["completion_tokens"].as_u64().unwrap_or(0) as u32,
        });

        let finish_reason = json["choices"][0]["finish_reason"].as_str().unwrap_or("stop").to_string();

        Ok(ModelResponse { text: content, parsed, usage, finish_reason })
    }
}
