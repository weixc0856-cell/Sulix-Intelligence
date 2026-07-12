//! Signal — 解释层（Intelligence 第一层输出）
//!
//! Signal 是"Intelligence 层对人类信号的第一遍解读"。
//! 它将纯事实（Observation）转换为带有判断的信息：
//!   - 重要性评分
//!   - 领域分类
//!   - 信号类别
//!   - 为什么这条信号重要
//!
//! 契约边界：
//!   Producer: Intelligence Engine (Signal Classification step)
//!   Consumer: Intelligence Engine (Theme Clustering step)

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 信号解释 — Intelligence 层的第一层输出
///
/// # 设计原则
/// - 每条 Signal 对应一条 Observation
/// - importance 是 0.0 ~ 1.0 的连续值（不是离散等级）
/// - domain 来自预定义的 StrategicDomain 列表
/// - why 必须是人可理解的推理链（不仅仅是分类标签）
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Signal {
    /// 唯一 ID，格式 "sig_xxx"
    pub id: String,

    /// 来源 Observation ID
    pub observation_id: String,

    /// 重要性评分 [0.0, 1.0]
    pub importance: f64,

    /// 战略领域，如 "AI Infrastructure"，"Semiconductor"
    pub domain: String,

    /// 信号类别
    pub category: SignalCategory,

    /// 为什么要关注这条信号（LLM 生成的推理）
    pub why: String,
}

/// 信号类别
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum SignalCategory {
    /// 结构性转变（范式级）
    StructuralShift,
    /// 竞争信号（对手动向）
    CompetitiveSignal,
    /// 上下文更新（环境变化）
    ContextUpdate,
    /// 噪声（低信息密度）
    Noise,
}
