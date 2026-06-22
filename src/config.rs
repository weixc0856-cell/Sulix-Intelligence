//! 配置模块 — TOML 配置加载

use anyhow::Result;
use serde::Deserialize;
use std::fs;

/// 根配置
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub llm: LlmConfig,
    pub output: OutputConfig,
    pub storage: Option<StorageConfig>,
    #[serde(default)]
    pub sources: Vec<SourceConfig>,
    /// Phase A: Scan Agent 配置
    #[serde(default)]
    pub scan_agent: Option<ScanAgentConfig>,
    /// Phase D: 记忆墓地配置
    #[serde(default)]
    pub graveyard: Option<GraveyardConfig>,
}

/// LLM 配置
#[derive(Debug, Deserialize, Clone)]
pub struct LlmConfig {
    pub api_key: Option<String>,
    /// Provider 名称: "deepseek" | "openai" | "perplexity"（默认 deepseek）
    #[serde(default = "default_llm_provider")]
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub max_tokens: u32,
    pub temperature: f32,
    /// Perplexity 专用 API Key（默认走 api_key）
    #[serde(default)]
    pub perplexity_key: Option<String>,
}

fn default_llm_provider() -> String {
    "deepseek".into()
}

/// 输出配置
#[derive(Debug, Deserialize, Clone)]
pub struct OutputConfig {
    pub vault_path: String,
    /// 日期范围过滤: "d1"/"d3"/"w1"/"w2"/"m1"（默认 "d7" = 7天）
    #[serde(default = "default_date_range")]
    pub date_range: String,
}

fn default_date_range() -> String {
    "d7".into()
}

/// 存储配置
#[derive(Debug, Deserialize, Clone)]
pub struct StorageConfig {
    pub data_dir: Option<String>,
}

/// RSS 源配置
#[derive(Debug, Deserialize, Clone)]
pub struct SourceConfig {
    pub name: String,
    /// 唯一的系统标识（ASCII only，不可用中文；不设置则自动从 name hash）
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub source_type: String,
    pub url: String,
    pub category: String,
    /// 正向关键词过滤（可选）：标题/摘要匹配任一即保留
    #[serde(default)]
    pub keywords: Option<Vec<String>>,
    /// 反向黑名单（可选）：标题/摘要匹配任一即熔断丢弃
    #[serde(default)]
    pub exclude_keywords: Option<Vec<String>>,
    /// 信息源层级：1=Signal, 2=Curated, 3=Community, 4=Market（预留）
    #[serde(default = "default_layer")]
    #[allow(dead_code)]
    pub layer: u8,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_layer() -> u8 {
    2
}

fn default_enabled() -> bool {
    true
}

/// Scan Agent 配置
#[derive(Debug, Deserialize, Clone)]
pub struct ScanAgentConfig {
    /// 是否启用 Scan Agent
    #[serde(default = "default_scan_enabled")]
    pub enabled: bool,
}

fn default_scan_enabled() -> bool {
    true
}

/// Phase D: 记忆墓地配置
#[derive(Debug, Deserialize, Clone)]
pub struct GraveyardConfig {
    /// 是否启用 Decay Agent
    #[serde(default = "default_graveyard_enabled")]
    pub enabled: bool,
    /// 文章保留天数（超过此天数进入墓地评估）
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    /// 是否启用 LLM 压缩
    #[serde(default = "default_compression_enabled")]
    pub compression: bool,
}

fn default_graveyard_enabled() -> bool {
    true
}
fn default_retention_days() -> u32 {
    90
}
fn default_compression_enabled() -> bool {
    true
}

impl Config {
    /// 从 TOML 文件加载配置
    pub fn from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("无法读取配置文件 {}: {}", path, e))?;
        let config: Config =
            toml::from_str(&content).map_err(|e| anyhow::anyhow!("TOML 解析错误: {}", e))?;
        Ok(config)
    }

    /// 获取当前 provider 的 API Key
    ///
    /// 按 provider 自动选择对应的 key 来源：
    /// - deepseek: config key → DEEPSEEK_API_KEY env
    /// - perplexity: perplexity_key → PERPLEXITY_API_KEY env
    /// - openai: config key → OPENAI_API_KEY env
    pub fn get_api_key(&self) -> Result<String> {
        let (env_var, config_fallback) = match self.llm.provider.as_str() {
            "perplexity" => ("PERPLEXITY_API_KEY", self.llm.perplexity_key.as_deref()),
            "openai" => ("OPENAI_API_KEY", self.llm.api_key.as_deref()),
            _ => ("DEEPSEEK_API_KEY", self.llm.api_key.as_deref()),
        };

        // 环境变量优先
        if let Ok(key) = std::env::var(env_var) {
            if !key.is_empty() {
                return Ok(key);
            }
        }
        // config.toml 兜底
        if let Some(key) = config_fallback {
            if !key.is_empty() {
                return Ok(key.to_string());
            }
        }
        Err(anyhow::anyhow!(
            "未找到 {} 的 API Key。请在 config.toml 中配置或设置 {} 环境变量。\
             config.toml 已在 .gitignore 中，不会上传 GitHub。",
            self.llm.provider,
            env_var
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_example() {
        let config = Config::from_file("config.example.toml").unwrap();
        assert_eq!(config.llm.model, "deepseek-v4-flash");
        assert!(config.sources.len() >= 2);
    }

    #[test]
    fn test_get_api_key_from_config() {
        let mut config = Config::from_file("config.example.toml").unwrap();
        config.llm.api_key = Some("test-key".into());
        let key = config.get_api_key().unwrap();
        assert_eq!(key, "test-key");
    }

    #[test]
    fn test_get_api_key_empty_fails() {
        let config = Config::from_file("config.example.toml").unwrap();
        assert!(config.llm.api_key.as_deref() == Some("") || config.llm.api_key.is_none());
        // config 中 key 为空，且测试环境下 DEEPSEEK_API_KEY 大概率未设置
        assert!(config.get_api_key().is_err());
    }
}
