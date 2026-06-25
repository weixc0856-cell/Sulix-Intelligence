//! 判断领域模型（Cognition Model 核心）
//!
//! Sulix 不是一个"分析新闻的系统"，而是一个"记录、验证、修正判断的系统"。
//! Thesis 是这一认知模型的核心——它不再只是"信念追踪"，而是"判断的完整生命周期"。
//!
//! 认知链路定位：
//!   信息输入 → 认知加工 → 判断评估（Thesis） → 决策行为 → 元思考
//!                                        ↑
//!                                    Thesis 在此
//!
//! 核心类型：Thesis（长期跟踪论题）、ThesisStatus（论题状态）、
//! ConfidenceSnapshot（置信度快照）、StatusTransition（状态变更记录）。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::domain::evidence::{Evidence, Stance};
use crate::domain::theme::Assumption;

/// Thesis 状态 — 完整生命周期
///
/// 内部状态机（前端展示为简化版本）:
///   Proposed → Active ⇄ Strengthening / Weakening → Dormant → Retired
///                                                          ↑ new evidence ↓
///                                                     (reactivate via Active)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ThesisStatus {
    /// 新建提案 — Hermes 提名，待用户确认
    Proposed,
    /// 常规跟踪
    Active,
    /// 近 7 天有 >= 2 条支持证据
    Strengthening,
    /// 近 7 天挑战证据 > 支持证据
    Weakening,
    /// 30 天无新证据
    Dormant,
    /// 90 天无新证据（或用户手动退休）
    Retired,
}

/// 一条长期跟踪的论题
///
/// Cognition Model 核心：记录一个判断从建立到修正的完整生命周期。
/// assumptions 是批判性思维的关键——大多数错误判断来自隐藏前提错误，
/// 而非事实错误。显式化 assumptions 使系统能追踪假设何时被证伪。
///
/// v2 新增:
/// - confidence_history: 事件驱动的置信度追踪（仅记录有意义的变化）
/// - status_history: 状态变更的时间线
/// - metadata: 扩展元数据（合并记录、复活事件等）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thesis {
    /// 唯一 ID
    pub id: String,
    /// 论题标题，如 "模型商品化"
    pub title: String,
    /// 首次记录日期
    pub created: String,
    /// 最后更新日期
    pub updated: String,
    /// 证据链（按时间排列）
    pub evidences: Vec<Evidence>,
    /// 承重假设（Cognition Model 一等公民）
    /// 记录该判断依赖的隐藏前提。当假设被证伪时，整个 Thesis 需重新评估。
    pub assumptions: Vec<Assumption>,
    /// 当前状态
    pub status: ThesisStatus,

    // === v2 新增字段 ===
    /// 事件驱动的置信度时间线（仅记录有意义的变化）
    #[serde(default)]
    pub confidence_history: Vec<ConfidenceSnapshot>,
    /// 状态变更历史
    #[serde(default)]
    pub status_history: Vec<StatusTransition>,
    /// 父论题 ID（合并/分叉时使用）
    #[serde(default)]
    pub parent_id: Option<String>,
    /// 已合并到此论题的 ID 列表
    #[serde(default)]
    pub merged_ids: Vec<String>,
    /// 相关论题 ID 列表
    #[serde(default)]
    pub related_thesis_ids: Vec<String>,
    /// 扩展元数据（复活事件、自定义标签等）
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// 置信度快照 — 事件驱动，非 daily sampling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceSnapshot {
    pub date: String,
    pub value: f64,
    pub trigger: ConfidenceTrigger,
    pub reason: String,
}

/// 置信度快照触发原因
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConfidenceTrigger {
    /// 首次创建
    Initial,
    /// 状态变更
    StatusChange,
    /// 置信度变化超过 10%
    SignificantChange,
    /// 用户手动更新
    ManualUpdate,
    /// 记录了 Outcome
    OutcomeRecorded,
}

/// 状态变更记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusTransition {
    pub from: ThesisStatus,
    pub to: ThesisStatus,
    pub date: String,
    pub trigger: TransitionTrigger,
    pub description: String,
}

/// 状态变更触发原因
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransitionTrigger {
    /// 证据数量/平衡触发
    EvidenceThreshold,
    /// 超时（Dormant/Retired）
    IdleTimeout,
    /// Hermes 发现
    HermesDetection,
    /// 用户手动操作
    UserAction,
    /// Outcome 触发
    OutcomeBased,
}

/// 信念声明
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefStatement {
    pub id: String,
    pub text: String,
    /// 当前置信度 1-10
    pub confidence: u8,
    pub category: String,
    /// 支撑该信念的证据 ID 列表
    pub evidence_ids: Vec<String>,
}

/// 信念更新记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefUpdate {
    pub belief_id: String,
    /// 置信度变化 (-10 to +10)
    pub delta: i8,
    pub evidence_type: Stance,
    pub reasoning: String,
    /// 是否为反向证伪信号
    pub is_contradiction: bool,
}

/// 信念数据库快照（Memory Layer 持久化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefDb {
    pub snapshot_date: String,
    pub beliefs: Vec<BeliefStatement>,
    pub recent_updates: Vec<BeliefUpdate>,
    pub total_support: usize,
    pub total_challenge: usize,
    pub contradictions_detected: usize,
}
