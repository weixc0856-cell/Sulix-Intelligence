//! Thesis — 可追踪的判断
//!
//! Thesis 是"我认为会发生什么"的具体判断。
//! 它是系统核心认知链路中的关键产物：
//!   - 可验证（在将来有明确的 "对/错" 判定）
//!   - 有时间边界
//!   - 有置信度
//!
//! 契约边界：
//!   Producer: Intelligence Engine (Thesis Generation step)
//!   Consumer: Intelligence Engine (Decision Mapping step)
//!             Memory (追踪、验证、反思)

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 可追踪判断 — 系统认知模型的核心产物
///
/// # 设计原则
/// - Thesis 必须可证伪（falsification_conditions 明确写出"什么情况下我错了"）
/// - time_horizon 给出判断的有效期（过了这个时间没有结果 = 自动 Pending）
/// - evidence 是 Signal ID 列表，形成完整证据链
/// - theme 和 belief_statement 是 Phase 2 内部字段，暂不实体化为独立步骤
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Thesis {
    /// 唯一 ID，格式 "thesis_xxx"
    pub id: String,

    /// 判断陈述（如 "GPU supply chain will tighten further in 2026 Q4"）
    pub claim: String,

    /// 当前置信度 [0.0, 1.0]
    pub confidence: f64,

    /// 支持证据（Signal ID 列表）
    #[serde(default)]
    pub evidence: Vec<String>,

    /// Thesis 状态
    pub status: ThesisStatus,

    /// 证伪条件 — 什么情况下这个判断是错误的
    /// 例如: ["企业 AI Agent 采用率连续 12 个月没有增长"]
    /// 这是 Reflection 判断"当时我到底预测了什么"的关键字段
    #[serde(default)]
    pub falsification_conditions: Vec<String>,

    /// 判断有效期 — 如 "12_months", "6_months", "30_days"
    /// 到期后自动标记为 Pending，等待 Outcome 确认/证伪
    #[serde(default = "default_time_horizon")]
    pub time_horizon: String,

    /// 主题名（Phase 2 内部字段，暂不实体化为 Theme 步骤）
    #[serde(default)]
    pub theme: Option<String>,

    /// 信念陈述（Phase 2 内部字段，暂不实体化为 Belief 步骤）
    #[serde(default)]
    pub belief_statement: Option<String>,
}

fn default_time_horizon() -> String {
    "12_months".into()
}

/// Thesis 生命周期状态
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum ThesisStatus {
    /// 新建提案
    Proposed,
    /// 常规跟踪
    Active,
    /// 近期有强化信号
    Strengthening,
    /// 近期有挑战信号
    Weakening,
    /// 待验证（到期可确认/证伪）
    Pending,
    /// 已确认（Outcome 验证为真）
    Confirmed,
    /// 已证伪（Outcome 验证为假）
    Invalidated,
}
