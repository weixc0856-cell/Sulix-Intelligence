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
    /// Phase A: Scan Agent 配置
    #[serde(default)]
    pub scan_agent: Option<ScanAgentConfig>,
    /// Phase B: 红蓝对抗 Agent 配置
    #[serde(default)]
    pub agent: Option<AgentConfig>,
    /// Phase D: 记忆墓地配置
    #[serde(default)]
    pub graveyard: Option<GraveyardConfig>,
    /// DecisionLedger: 活跃决策问题
    #[serde(default)]
    pub decisions: Option<DecisionLedger>,
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

/// Scan Agent 配置
#[derive(Debug, Deserialize, Clone)]
pub struct ScanAgentConfig {
    /// 是否启用 Scan Agent
    #[serde(default = "default_scan_enabled")]
    pub enabled: bool,
    /// 重要性阈值，≤此值的文章被跳过（默认 3）
    #[serde(default = "default_scan_threshold")]
    pub threshold: u8,
}

fn default_scan_enabled() -> bool {
    true
}
fn default_scan_threshold() -> u8 {
    3
}

/// Phase B: 红蓝对抗 Agent 配置
#[derive(Debug, Deserialize, Clone)]
pub struct AgentConfig {
    /// 是否启用 Synthesis Agent（红军）
    #[serde(default = "default_agent_enabled")]
    pub synthesis_enabled: bool,
    /// 是否启用 Verification Agent（蓝军）
    #[serde(default = "default_agent_enabled")]
    pub verification_enabled: bool,
}

fn default_agent_enabled() -> bool {
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
    /// 埋葬阈值（重要性 ≤ 此值即埋葬）
    #[allow(dead_code)]
    #[serde(default = "default_burial_threshold")]
    pub burial_threshold: u8,
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
fn default_burial_threshold() -> u8 {
    3
}

/// DecisionLedger — 追踪活跃决策及其证据状态
#[derive(Debug, Deserialize, Clone)]
pub struct DecisionLedger {
    #[serde(default)]
    pub decisions: Vec<Decision>,
}

/// 单条决策问题（可回答、具体、可操作）
#[derive(Debug, Deserialize, Clone)]
pub struct Decision {
    pub id: String,
    pub question: String,
    #[serde(default = "default_decision_status")]
    pub status: String,     // "active" | "archived"
    #[serde(default = "default_decision_position")]
    pub position: String,   // "pro" | "con" | "neutral"
}

fn default_decision_status() -> String { "active".into() }
fn default_decision_position() -> String { "neutral".into() }

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
        let config: Config =
            toml::from_str(&content).map_err(|e| anyhow::anyhow!("TOML 解析错误: {}", e))?;
        Ok(config)
    }

    /// 获取 DeepSeek API Key
    ///
    /// 从 config.toml 读取（已在 .gitignore 中，不会上传 GitHub）。
    /// 环境变量 DEEPSEEK_API_KEY 可作为替代。
    pub fn get_api_key(&self) -> Result<String> {
        // config.toml 优先（推荐使用方式——Key 只在本地）
        if let Some(key) = &self.llm.api_key {
            if !key.is_empty() {
                return Ok(key.clone());
            }
        }
        // 环境变量作为替代方案
        if let Ok(key) = std::env::var("DEEPSEEK_API_KEY") {
            if !key.is_empty() {
                return Ok(key);
            }
        }
        Err(anyhow::anyhow!(
            "请在 config.toml 中填写 api_key，或设置 DEEPSEEK_API_KEY 环境变量。\
             config.toml 已在 .gitignore 中，不会上传 GitHub。"
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
