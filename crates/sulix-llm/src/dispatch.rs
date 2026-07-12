//! LLM Provider 派发 — 类比 ripgrep `PatternMatcher`

use sulix_config::LlmConfig;

use crate::api::call_with_retry_raw;
use crate::client::create_client;

/// LLM Provider 派发枚举 — 编译期或运行时可选择的 LLM 引擎
///
/// 类比 ripgrep 的 `PatternMatcher`（Enum dispatch 模式）：
/// 将不同 LLM provider 包装为 enum variant，调用者通过 match 派发，
/// 无需 trait object 或 dynamic dispatch。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LlmProviderDispatch {
    DeepSeek,
    OpenAI,
    Perplexity,
}

impl LlmProviderDispatch {
    /// 从配置创建 Provider 派发器
    pub fn from_config(config: &LlmConfig) -> Self {
        match config.provider.as_str() {
            "openai" => Self::OpenAI,
            "perplexity" => Self::Perplexity,
            _ => Self::DeepSeek,
        }
    }

    /// 获取 provider 名称
    pub fn name(&self) -> &'static str {
        match self {
            Self::DeepSeek => "deepseek",
            Self::OpenAI => "openai",
            Self::Perplexity => "perplexity",
        }
    }

    /// 调用 LLM 并返回原始文本（带重试）
    pub async fn call(
        &self,
        api_key: &str,
        llm_config: &LlmConfig,
        system_prompt: &str,
        user_prompt: &str,
    ) -> anyhow::Result<String> {
        let client = create_client(120)?;
        call_with_retry_raw(&client, api_key, llm_config, system_prompt, user_prompt).await
    }
}

/// Auto-choice 模式 — 类比 ripgrep `EngineChoice::Auto`
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LlmChoice {
    Auto,
    DeepSeek,
    OpenAI,
    Perplexity,
}

impl LlmChoice {
    pub fn resolve(&self, config: &LlmConfig) -> LlmProviderDispatch {
        match self {
            Self::DeepSeek => LlmProviderDispatch::DeepSeek,
            Self::OpenAI => LlmProviderDispatch::OpenAI,
            Self::Perplexity => LlmProviderDispatch::Perplexity,
            Self::Auto => LlmProviderDispatch::from_config(config),
        }
    }
}
