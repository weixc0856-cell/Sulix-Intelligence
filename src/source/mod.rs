//! Source Adapter 模块（抄 RSSHub: 每源一个适配器，统一 RawSignal 输出）
//!
//! fetch_source() 根据 source_type 路由到对应的适配器。
//! 加新源：在 match 中增加一个分支即可。
//! 不使用 trait（避免 async_trait 依赖），直接用函数分发。

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, FixedOffset};
use serde::Serialize;

use crate::config::SourceConfig;

pub mod rss;
pub mod uspto;

/// 统一信号结构（RSSHub DataItem 对应，含可选的数字指标字段）
#[derive(Debug, Clone, Serialize)]
pub struct RawSignal {
    pub id: String,
    pub title: String,
    pub url: String,
    pub content: Option<String>,
    pub summary: Option<String>,
    pub published_at: Option<DateTime<FixedOffset>>,
    pub source: String,
    pub source_id: String,
    pub category: String,
    /// 数字指标（A 股换手率/主力流向等硬数据）
    pub metrics: Option<HashMap<String, String>>,
    /// Layer 2 敏感标记：true = 需要翻译车间洗白
    #[serde(default)]
    pub requires_sanitization: bool,
    /// 是否为内参学习源（layer == 1）
    /// 内参源仅用于后台 LLM 认知校准，前端不展示溯源链接
    pub is_internal: bool,
}

/// 分发到对应适配器并执行抓取
pub async fn fetch_source(config: &SourceConfig, date_range: &str) -> Result<Vec<RawSignal>> {
    match config.source_type.as_str() {
        "rss" => rss::fetch_rss(config, date_range).await,
        "uspto" => uspto::fetch_patents(config, date_range).await,
        other => Err(anyhow::anyhow!("未知源类型: {}", other)),
    }
}

/// 替换 RSSHub URL 中的实例地址
/// 所有 rsshub.app 开头的 URL 以配置的 rsshub_base 替换
pub fn resolve_rsshub_url(url: &str, rsshub_base: &str) -> String {
    if url.contains("rsshub.app") && rsshub_base != "https://rsshub.app" {
        url.replace("https://rsshub.app", rsshub_base)
    } else {
        url.to_string()
    }
}

/// 构建可展示 attribution 链接的源名称集合
///
/// 仅包含 `show_attribution() == true` 的源（public == true 且 layer != 1）。
/// 用于前端渲染时过滤内参源和私有源。
pub fn attributable_source_names(sources: &[SourceConfig]) -> HashSet<String> {
    sources
        .iter()
        .filter(|s| s.show_attribution())
        .map(|s| s.name.clone())
        .collect()
}

/// 从 Vault 的 .flash/ 目录加载人工注入的特殊专题
///
/// Code Review 防御性设计:
/// - 只读取 .json 文件，过滤 .DS_Store/.trash 等临时文件
/// - 解析失败的文件被跳过而非崩溃
/// - start_date/end_date 用于控制专题有效期（Phase 3 实现日期过滤）
pub fn load_special_topics(vault_path: &str) -> Vec<crate::premium::SpecialTopic> {
    let flash_path = Path::new(vault_path).join(".flash");
    if !flash_path.exists() {
        return vec![];
    }
    let mut topics = vec![];
    if let Ok(entries) = std::fs::read_dir(&flash_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            // 只读取 .json 文件，防止 Obsidian 临时文件导致反序列化崩溃
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(topic) = serde_json::from_str::<crate::premium::SpecialTopic>(&content) {
                    topics.push(topic);
                }
            }
        }
    }
    topics
}
