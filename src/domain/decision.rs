//! 决策领域模型 — Decision Intelligence 输出类型
//!
//! Thesis → Decision 映射的输出类型。确定性映射规则（非 LLM）。
//!
//! TODO: 在 Phase 2 中统一 DecisionState / DecisionTransition / DecisionRecord
//!       与 ThesisDecision 之间的关系，消除重复建模。

use serde::{Deserialize, Serialize};
use crate::domain::action::{DecisionHorizon, DecisionStability, DecisionType};

// ===== Canonical Decision Object (DEC-XXXX) =====

/// Administrative lifecycle state of a canonical Decision object
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(tag = "state", content = "detail")]
pub enum DecisionState {
    #[default]
    Active,
    Archived { reason: String },
    Superseded { by: String }, // by DEC-XXXX
    Expired,
}

/// Full transition event: from → to (richer than DecisionSnapshot)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionTransition {
    pub date: String,
    /// Previous decision_type label or "initial"
    pub from: String,
    /// New decision_type label
    pub to: String,
    pub confidence: f64,
    /// What triggered the change: "evidence-update", "outcome", "manual"
    pub trigger: String,
}

/// Canonical Decision object with stable DEC-XXXX ID
///
/// One DEC per active Assessment. ID is stable; decision_type evolves.
/// Links to ASM-XXXX (primary), thesis_id (internal reference only).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    /// Stable canonical ID, e.g. "DEC-0001"
    pub id: String,
    /// Primary link to canonical Assessment (ASM-XXXX)
    pub asm_id: String,
    /// Internal reference — thesis-XXXX ephemeral ID
    pub thesis_id: String,
    /// Current decision type label (lowercase): "build", "monitor", etc.
    pub decision_type: String,
    /// Time horizon string: "30d", "90d", "180d", "immediate"
    pub horizon: String,
    pub confidence: f64,
    pub rationale: String,
    /// Stability label: "volatile", "stable", "final"
    pub stability: String,
    #[serde(default)]
    pub state: DecisionState,
    pub created: String,
    pub updated: String,
    /// Reserved for OUT-XXXX links (future)
    #[serde(default)]
    pub outcome_ids: Vec<String>,
    /// Type transition log (from → to events)
    #[serde(default)]
    pub decision_history: Vec<DecisionTransition>,
}

/// Thesis → Decision 映射结果
#[derive(Debug, Clone)]
pub struct ThesisDecision {
    pub thesis_id: String,
    pub thesis_title: String,
    pub decision_type: DecisionType,
    /// 决策置信度（基于 evidence ratio），当前未在前端使用但保留用于未来 filtering
    pub confidence: f64,
    pub rationale: String,
    pub horizon: DecisionHorizon,
    pub priority: u8,
    /// 决策稳定性 — outcome history 驱动
    pub stability: DecisionStability,
}
