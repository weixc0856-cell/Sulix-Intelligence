//! IntelligenceEvent — 系统中唯一的状态变化来源
//!
//! 所有认知状态（Thesis、Decision、Signal）的变化都记录为不可变事件。
//! State projection 从事件流派生，而非直接修改。
//!
//! 架构原则（ADR-011）:
//!   Event Store 作为 Truth Source
//!   State Projections 作为读取优化
//!
//! 类比：
//!   - Event = Git commit（不可变，有作者、时间、原因）
//!   - Projection = Git branch（从 commit 派生，可重建）
//!
//! 契约边界：
//!   Producer: Intelligence Engine (Pipeline steps)
//!   Consumer: Event Store (sulix-store), Timeline API

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 认知事件 — 系统中唯一的状态变化来源
///
/// # 事件类型
/// 每个 aggregate type 有对应的事件类型集：
///
/// ## Signal
/// - "SignalClassified" — 新信号产生
/// - "SignalUpdated" — 信号信息更新
///
/// ## Thesis
/// - "ThesisProposed" — 新判断提案
/// - "EvidenceAttached" — 证据追加
/// - "ConfidenceChanged" — 置信度变化
/// - "StatusChanged" — 状态变化（Active→Strengthening 等）
/// - "FalsificationAdded" — 证伪条件追加
///
/// ## Decision
/// - "DecisionGenerated" — 新决策生成
/// - "DecisionChanged" — 决策变化（Monitor→Invest 等）
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IntelligenceEvent {
    /// 事件 ID，格式 "evt_xxx"
    pub id: String,

    /// 聚合类型: "signal", "thesis", "decision"
    pub aggregate_type: String,

    /// 聚合 ID（如 thesis_001, sig_001）
    pub aggregate_id: String,

    /// 事件类型: "SignalClassified", "ThesisProposed", "DecisionGenerated"
    pub event_type: String,

    /// 事件载荷（任意 JSON，按 event_type 解析）
    pub payload: serde_json::Value,

    /// 事件来源步骤: "signal_classification", "thesis_generation", "decision_mapping"
    #[serde(default)]
    pub source: String,

    /// 创建时间（ISO 8601）
    pub created_at: String,
}

impl IntelligenceEvent {
    /// 创建新事件
    pub fn new(
        aggregate_type: &str,
        aggregate_id: &str,
        event_type: &str,
        payload: serde_json::Value,
        source: &str,
    ) -> Self {
        let ts = chrono::Utc::now();
        // Microsecond precision ensures uniqueness within the same process
        let id = format!("evt_{}_{}", ts.format("%Y%m%d%H%M%S%6f"), aggregate_id);
        Self {
            id,
            aggregate_type: aggregate_type.to_string(),
            aggregate_id: aggregate_id.to_string(),
            event_type: event_type.to_string(),
            payload,
            source: source.to_string(),
            created_at: ts.to_rfc3339(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = IntelligenceEvent::new(
            "thesis",
            "thesis_001",
            "ThesisProposed",
            serde_json::json!({
                "claim": "AI Agent adoption will accelerate",
                "confidence": 0.72
            }),
            "thesis_generation",
        );
        assert!(event.id.starts_with("evt_"));
        assert_eq!(event.aggregate_type, "thesis");
        assert_eq!(event.event_type, "ThesisProposed");
        assert_eq!(event.payload["claim"], "AI Agent adoption will accelerate");
    }

    #[test]
    fn test_event_unique_ids() {
        let e1 = IntelligenceEvent::new("thesis", "t1", "ThesisProposed", serde_json::json!({}), "test");
        let e2 = IntelligenceEvent::new("thesis", "t1", "ThesisProposed", serde_json::json!({}), "test");
        assert_ne!(e1.id, e2.id, "events should have unique IDs");
    }

    #[test]
    fn test_event_serde_roundtrip() {
        let event = IntelligenceEvent::new(
            "decision",
            "dec_001",
            "DecisionGenerated",
            serde_json::json!({
                "action": "invest",
                "confidence": 0.7,
                "reasoning": "Strong signals"
            }),
            "decision_mapping",
        );
        let json = serde_json::to_string(&event).unwrap();
        let restored: IntelligenceEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.event_type, "DecisionGenerated");
        assert_eq!(restored.payload["action"], "invest");
    }
}
