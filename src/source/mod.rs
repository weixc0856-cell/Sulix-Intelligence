//! Source Adapter 模块（抄 RSSHub: 每源一个适配器，统一 RawSignal 输出）
//!
//! dispatch_source() 根据配置路由到对应的适配器。
//! 不使用 trait（避免 async_trait 依赖），直接用函数分发。

use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, FixedOffset};
use serde::Serialize;

use crate::config::SourceConfig;

pub mod rss;

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
}

/// 分发到对应适配器并执行抓取
pub async fn fetch_source(config: &SourceConfig) -> Result<Vec<RawSignal>> {
    match config.source_type.as_str() {
        "rss" => rss::fetch_rss(config).await,
        other => Err(anyhow::anyhow!("未知源类型: {}", other)),
    }
}
