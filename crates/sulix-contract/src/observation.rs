//! Observation — 纯事实层（感官）
//!
//! Observation 是 Intelligence 系统的"视网膜"。
//! 它只记录"发生了什么"，不做任何解释。
//! 不包含 category/tags/importance/domain 等判断性字段。
//!
//! 契约边界：
//!   Producer: RSS 源适配器 (source::*)
//!   Consumer: Intelligence Engine (只有它有权消费 Observation)
//!
//! 去重策略：
//!   通过 content_hash + source_id 的组合判断重复。
//!   同一来源的内容相同 = 重复；不同来源的内容相同 = 交叉验证信号（non-trivial）。

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 纯事实观察 — 系统的最小输入单元
///
/// # 设计原则
/// - 不包含任何解释性字段（importance/domain/category）
/// - entities 仅做命名实体识别，不做情感/重要性判断
/// - raw_content 保留原文，不做摘要/截断
/// - content_hash + source_id 构成唯一性，支持跨源去重
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Observation {
    /// 唯一 ID，格式 "obs_xxx"
    pub id: String,

    /// 标题
    pub title: String,

    /// 来源名称，如 "OpenAI Blog", "Hacker News"
    pub source: String,

    /// 来源内部 ID（如 RSS guid），用于跨实例去重
    #[serde(default)]
    pub source_id: String,

    /// 原文 URL
    pub url: String,

    /// 发布时间（ISO 8601）
    pub published_at: String,

    /// 捕获时间（ISO 8601）— 系统何时捕获到这条信息
    pub captured_at: String,

    /// 内容 SHA256 哈希 — 用于内容级去重
    pub content_hash: String,

    /// 原文内容（未截断，保留完整）
    pub raw_content: String,

    /// 命名实体列表（纯识别，不含分类）
    #[serde(default)]
    pub entities: Vec<String>,
}
