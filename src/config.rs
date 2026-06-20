//! 配置模块 — TOML 配置加载

use anyhow::Result;
use serde::Deserialize;
use std::fs;

/// 根配置
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub llm: LlmConfig,
    pub output: OutputConfig,
    #[allow(dead_code)]
    pub dedup: DedupConfig,
    pub storage: Option<StorageConfig>,
    #[serde(default)]
    pub sources: Vec<SourceConfig>,
    #[allow(dead_code)]
    pub prompts: PromptConfig,
}

/// LLM 配置
#[derive(Debug, Deserialize, Clone)]
pub struct LlmConfig {
    pub api_key: Option<String>,
    pub model: String,
    pub base_url: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

/// 输出配置
#[derive(Debug, Deserialize, Clone)]
pub struct OutputConfig {
    pub vault_path: String,
}

/// 去重配置（预留，当前在 dedup_and_insert 中使用精确去重）
#[derive(Debug, Deserialize, Clone)]
pub struct DedupConfig {
    #[allow(dead_code)]
    pub window_hours: u32,
    #[allow(dead_code)]
    pub title_similarity_threshold: f32,
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
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub source_type: String,
    pub url: String,
    pub category: String,
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

/// Prompt 配置（预留，当前 prompt 直接写在 config.toml 中由用户自定义）
#[derive(Debug, Deserialize, Clone)]
pub struct PromptConfig {
    #[allow(dead_code)]
    pub base: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub vertical_overrides: std::collections::HashMap<String, String>,
}

impl Config {
    /// 从 TOML 文件加载配置
    pub fn from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("无法读取配置文件 {}: {}", path, e))?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("TOML 解析错误: {}", e))?;
        Ok(config)
    }

    /// 获取 DeepSeek API Key
    pub fn get_api_key(&self) -> Result<String> {
        if let Some(key) = &self.llm.api_key {
            if !key.is_empty() {
                return Ok(key.clone());
            }
        }
        std::env::var("DEEPSEEK_API_KEY")
            .map_err(|_| anyhow::anyhow!("未设置 DEEPSEEK_API_KEY 环境变量或在 config.toml 中配置"))
    }
}
