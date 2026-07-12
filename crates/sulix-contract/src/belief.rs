//! Belief — 对世界的基本假设
//!
//! Belief 是系统的"世界观核心"。
//! 不同于 Thesis（具体判断），Belief 是更底层的假设：
//!   - "GPU 供应链将持续紧张到 2027"
//!   - "AI Agent 将在 2 年内替代部分软件开发流程"
//!
//! Belief 是系统长期积累的真正资产。
//! 当 Belief 被证伪，系统发生"认知更新"。
//!
//! 契约边界：
//!   Producer: Intelligence Engine (Belief Assessment step)
//!   Consumer: Intelligence Engine (Thesis Generation step)
//!             Memory (长期维护、更新)

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 对世界的基本假设 — BeliefEngine 核心类型
///
/// # 设计原则
/// - Belief 比 Thesis 更抽象、更底层
/// - 一个 Belief 可以派生多个 Thesis
/// - 当 Belief 被证伪，所有相关 Thesis 自动标记为需要重新评估
/// - evidence_for / evidence_against 是不断累积的
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Belief {
    /// 唯一 ID，格式 "bel_xxx"
    pub id: String,

    /// 信念陈述（如 "GPU supply will remain constrained through 2027"）
    pub statement: String,

    /// 当前置信度 [0.0, 1.0]
    pub confidence: f64,

    /// 支持证据（指向 Signal ID 列表）
    #[serde(default)]
    pub evidence_for: Vec<String>,

    /// 反对证据（指向 Signal ID 列表）
    #[serde(default)]
    pub evidence_against: Vec<String>,

    /// 最后修订日期（ISO 8601）
    pub last_revised: String,
}
