//! Theme — 长期认知节点
//!
//! Theme 不是"这组文章的标题"，而是长期存在的认知结构。
//! Theme 可以进入 Memory，跨天累积。
//!
//! 例如：
//!   Observation: "NVIDIA 发布新 GPU"
//!   Signal: "AI 算力需求持续增长"
//!   Theme: "AI Infrastructure Scaling" ← 长期存在
//!
//! 契约边界：
//!   Producer: Intelligence Engine (Theme Clustering step)
//!   Consumer: Intelligence Engine (Belief Assessment step)
//!             Memory (长期积累)

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 长期认知节点
///
/// # 设计原则
/// - Theme 是跨天的累积概念，不是每日临时分组
/// - status 反映主题的活跃度
/// - related_signals 链接到 Signal，形成证据链
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Theme {
    /// 唯一 ID，格式 "theme_xxx"
    pub id: String,

    /// 主题名称（人类可读，如 "AI Infrastructure Scaling"）
    pub name: String,

    /// 主题状态
    pub status: ThemeStatus,

    /// 首次观察日期（ISO 8601）
    pub first_observed: String,

    /// 最后更新日期（ISO 8601）
    pub last_updated: String,

    /// 相关 Signal ID 列表
    #[serde(default)]
    pub related_signals: Vec<String>,

    /// 简要摘要（跨信号累积）
    #[serde(default)]
    pub summary: String,
}

/// 主题状态
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum ThemeStatus {
    /// 活跃（7 天内有新信号）
    Active,
    /// 休眠（7-30 天无新信号）
    Dormant,
    /// 归档（30 天以上无新信号）
    Archived,
}
