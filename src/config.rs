//! 配置模块 — TOML 配置加载

use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

/// 根配置
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
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
    /// Phase 2: 可配置的 Prompts（每个字段为 Option，None = 使用代码中的硬编码默认值）
    #[serde(default)]
    pub prompts: Option<PromptsConfig>,
    /// Phase 2: Substack 发布配置
    #[serde(default)]
    pub substack: Option<SubstackConfig>,
    /// Phase 2: 用户关切问题系统
    #[serde(default)]
    pub questions: Option<QuestionsConfig>,
    /// News Layer 配置
    #[serde(default)]
    pub news_layer: Option<NewsLayerConfig>,
    /// 去重配置
    #[serde(default)]
    pub dedup: Option<DedupConfig>,
    /// Twitter/X 发布配置
    #[serde(default)]
    pub twitter: Option<TwitterConfig>,
    /// Belief Engine Phase B: WayneOPC 核心信念
    #[serde(default)]
    pub beliefs: Option<Vec<BeliefConfig>>,
}

/// LLM 配置
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct OutputConfig {
    pub vault_path: String,
    /// 日期范围过滤: "d1"/"d3"/"w1"/"w2"/"m1"（默认 "d7" = 7天）
    #[serde(default = "default_date_range")]
    pub date_range: String,
    /// MDX 输出目录（如 "output/"），None = 不生成 MDX
    #[serde(default)]
    pub mdx_dir: Option<String>,
}

fn default_date_range() -> String {
    "d7".into()
}

/// 存储配置
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    pub data_dir: Option<String>,
}

/// RSS 源配置
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SourceConfig {
    pub name: String,
    /// 唯一的系统标识（ASCII only，不可用中文；不设置则自动从 name hash）
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub source_type: String,
    pub url: String,
    pub category: String,
    /// 正向关键词过滤（可选）：标题/摘要匹配任一即保留
    #[serde(default)]
    pub keywords: Option<Vec<String>>,
    /// 反向黑名单（可选）：标题/摘要匹配任一即熔断丢弃
    #[serde(default)]
    pub exclude_keywords: Option<Vec<String>>,
    /// 信息源层级：1=内参学习（不挂公开链接）, 2=官方权威源, 3=极客社区, 4=市场数据
    #[serde(default = "default_layer")]
    pub layer: u8,
    /// 是否公开可展示。false 时前端不显示该源的 attribution 链接，但 LLM 仍完全吸收
    #[serde(default = "default_public")]
    pub public: bool,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 源权重 1-10：10=官方权威源（OpenAI Blog, BIS），1=社交噪音（Reddit, Twitter）
    /// 用于 SVI 计算中的 source_score 因子替代 layer-based 映射
    #[serde(default = "default_source_score")]
    pub score: u8,
}

impl SourceConfig {
    /// 是否为内参学习源（layer == 1）
    /// 内参源仅用于后台 LLM 认知校准，前端不展示溯源链接
    pub fn is_internal(&self) -> bool {
        self.layer == 1
    }

    /// 是否在前端展示 attribution 链接
    /// 仅当 public == true 且不为内参源（layer != 1）时才展示
    pub fn show_attribution(&self) -> bool {
        self.public && self.layer != 1
    }
}

fn default_layer() -> u8 {
    2
}

fn default_source_score() -> u8 {
    5
}

fn default_enabled() -> bool {
    true
}

fn default_public() -> bool {
    true
}

/// Scan Agent 配置
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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

/// Phase 2: 可配置的 Prompts 系统
///
/// 每个字段为 Option<String>，None = 使用代码中的硬编码默认值。
/// 通过 accessor 方法（get_*）传入默认值，由调用方在各自的模块中维护。
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct PromptsConfig {
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub vertical_overrides: Option<HashMap<String, String>>,
    #[serde(default)]
    pub scan_agent: Option<String>,
    #[serde(default)]
    pub calibration: Option<String>,
    #[serde(default)]
    pub cluster_articles: Option<String>,
    #[serde(default)]
    pub analyze_theme: Option<String>,
    #[serde(default)]
    pub challenge_theme: Option<String>,
    #[serde(default)]
    pub diplomat: Option<String>,
    #[serde(default)]
    pub architect: Option<String>,
    #[serde(default)]
    pub quant: Option<String>,
}

#[allow(dead_code)]
impl PromptsConfig {
    pub fn get_scan_agent<'a>(&'a self, default: &'a str) -> &'a str {
        self.scan_agent.as_deref().unwrap_or(default)
    }
    pub fn get_calibration<'a>(&'a self, default: &'a str) -> &'a str {
        self.calibration.as_deref().unwrap_or(default)
    }
    pub fn get_cluster_articles<'a>(&'a self, default: &'a str) -> &'a str {
        self.cluster_articles.as_deref().unwrap_or(default)
    }
    pub fn get_analyze_theme<'a>(&'a self, default: &'a str) -> &'a str {
        self.analyze_theme.as_deref().unwrap_or(default)
    }
    pub fn get_challenge_theme<'a>(&'a self, default: &'a str) -> &'a str {
        self.challenge_theme.as_deref().unwrap_or(default)
    }
    pub fn get_diplomat<'a>(&'a self, default: &'a str) -> &'a str {
        self.diplomat.as_deref().unwrap_or(default)
    }
    pub fn get_architect<'a>(&'a self, default: &'a str) -> &'a str {
        self.architect.as_deref().unwrap_or(default)
    }
    pub fn get_quant<'a>(&'a self, default: &'a str) -> &'a str {
        self.quant.as_deref().unwrap_or(default)
    }
}

/// Phase 2: Substack 发布配置
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SubstackConfig {
    /// Substack API Key
    pub api_key: String,
    /// Substack 出版物 URL: "https://sulix.substack.com"
    pub publication_url: String,
    /// 是否启用自动推送
    #[serde(default = "default_substack_enabled")]
    pub enabled: bool,
}

fn default_substack_enabled() -> bool {
    false // 默认不启用，避免意外推送测试数据
}

/// Phase 2: 用户关切问题系统
///
/// 在 config.toml 的 [questions] 段中声明。
/// Question 的 text 字段在 TOML 中为 "question"。
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct QuestionsConfig {
    #[serde(default)]
    pub questions: Vec<crate::question_engine::Question>,
}

/// News Layer 配置
#[derive(Debug, Deserialize, Clone)]
pub struct NewsLayerConfig {
    /// 聚类前启用 LLM 语义去重
    #[serde(default = "default_llm_prededup")]
    pub llm_prededup: bool,
    /// LLM 预去重批大小
    #[serde(default = "default_prededup_batch")]
    pub prededup_batch_size: usize,
    /// Change Detection 使用 LLM 语义版（否则用规则版）
    #[serde(default = "default_llm_change_detection")]
    pub llm_change_detection: bool,
}

fn default_llm_prededup() -> bool {
    false
}
fn default_prededup_batch() -> usize {
    15
}
fn default_llm_change_detection() -> bool {
    false
}

/// 去重配置
#[derive(Debug, Deserialize, Clone)]
pub struct DedupConfig {
    /// Jaccard 标题相似度阈值（超过此值判定为重复，默认 0.75）
    #[serde(default = "default_dedup_threshold")]
    pub title_similarity_threshold: f64,
    /// 去重时间窗口（小时），0 表示不限制
    #[serde(default = "default_dedup_window")]
    #[allow(dead_code)]
    pub window_hours: u32,
}

fn default_dedup_threshold() -> f64 {
    0.75
}
fn default_dedup_window() -> u32 {
    0
}

/// Belief Engine Phase B: WayneOPC 核心信念配置
#[derive(Debug, Deserialize, Clone)]
pub struct BeliefConfig {
    /// 信念 ID (B1-B10)
    pub id: String,
    /// 信念陈述
    pub statement: String,
    /// 初始置信度 0-100
    pub confidence: u8,
    /// 类别
    pub category: String,
}

/// Twitter/X 发布配置
#[derive(Debug, Deserialize, Clone)]
pub struct TwitterConfig {
    /// API Bearer Token
    pub bearer_token: String,
    /// 是否启用
    #[serde(default)]
    pub enabled: bool,
}

impl Config {
    /// 从 TOML 文件加载配置
    pub fn from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("无法读取配置文件 {}: {}", path, e))?;
        let config: Config =
            toml::from_str(&content).map_err(|e| anyhow::anyhow!("TOML 解析错误: {}", e))?;
        // 验证 date_range 格式
        let valid = &["d1", "d3", "d7", "w1", "w2", "m1"];
        if !valid.contains(&config.output.date_range.as_str()) {
            anyhow::bail!(
                "无效的 date_range '{}'，预期值: {}",
                config.output.date_range,
                valid.join(", ")
            );
        }
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
