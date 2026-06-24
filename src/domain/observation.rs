//! 原始观察领域模型
//!
//! Observation 是认知链路的第一环——"原子级信号"。
//! 它捕获原始信号在成为 Theme 之前的初始状态，是"到底发生了什么"的不可变记录。
//!
//! 认知链路定位：
//!   信息输入（Source Layer）→ Observation → 认知加工（AnalysisEngine）→ ...
//!                              ↑
//!                         Observation 在此：原始信号的原子记录
//!
//! Observation 与 Article 的区别：
//!   - Observation 是认知抽象的起点（"我注意到 X 发生了"）
//!   - Article 是物理数据的载体（"某篇文章如此报道"）
//!   一个 Observation 可以对应多篇 Article，一篇 Article 可以产生多个 Observation。

use serde::{Deserialize, Serialize};

/// 原始观察：认知链路的原子输入
///
/// Observation 不是"标题"或"摘要"，而是"观察者注意到什么变化"。
/// 例如："OpenAI 发布 Agent SDK" 是一个 Observation，
///        而 "TechCrunch 报道 OpenAI 发布 Agent SDK" 才是 Article。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// 唯一 ID
    pub id: String,
    /// 观察来源（URL / pipeline stage name / user input）
    pub source: String,
    /// 观察发生时间
    pub timestamp: String,
    /// 观察内容——"到底发生了什么"
    pub content: String,
    /// 初始信号强度 1-10（观察者的主观评分）
    pub signal_strength: u8,
    /// 关联的实体名称列表
    #[serde(default)]
    pub entity_refs: Vec<String>,
    /// 原始来源 URL（如果有）
    #[serde(default)]
    pub source_url: Option<String>,
}
