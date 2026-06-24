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
//! BeliefStatement（信念声明）、BeliefUpdate（信念更新）、
//! BeliefDb（信念数据库快照）。

use serde::{Deserialize, Serialize};

use crate::domain::evidence::{Evidence, Stance};
use crate::domain::theme::Assumption;

/// Thesis 状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ThesisStatus {
    /// 常规跟踪
    Active,
    /// 近 7 天有 >= 2 条支持证据
    Strengthening,
    /// 近 7 天挑战证据 > 支持证据
    Weakening,
    /// 连续 30 天无新证据
    Retired,
}

/// 一条长期跟踪的论题
///
/// Cognition Model 核心：记录一个判断从建立到修正的完整生命周期。
/// assumptions 是批判性思维的关键——大多数错误判断来自隐藏前提错误，
/// 而非事实错误。显式化 assumptions 使系统能追踪假设何时被证伪。
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
