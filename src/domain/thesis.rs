//! 论题领域模型
//!
//! 核心类型：Thesis（长期跟踪论题）、ThesisStatus（论题状态）、
//! BeliefStatement（信念声明）、BeliefUpdate（信念更新）、
//! EvidenceType（证据类型）、BeliefDb（信念数据库快照）。

use serde::{Deserialize, Serialize};

use crate::domain::evidence::Evidence;

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
    pub evidence_type: EvidenceType,
    pub reasoning: String,
    /// 是否为反向证伪信号
    pub is_contradiction: bool,
}

/// 证据类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvidenceType {
    /// 新证据支持该信念
    Support,
    /// 新证据挑战该信念
    Challenge,
    /// 无关
    Neutral,
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
