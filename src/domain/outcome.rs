//! 结果领域模型
//!
//! Outcome 记录决策（DEC）的验证结果——"我的判断对了吗？"
//! 这是认知链路从"判断"到"结果验证"的关键闭环。
//!
//! 认知链路定位：
//!   ... → Decision（决策）→ Outcome（结果）→ Reflection（复盘）
//!
//! v4 变更：
//!   - 新增 decision_id（必填，关联 DEC-XXXX）
//!   - 新增 impact（Low/Medium/High）
//!   - 保留 thesis_id（reflection 回溯 thesis 需要）

use serde::{Deserialize, Serialize};
use crate::event_log::{ObjectEvent, ObjectEventType};

/// 生成 OUT-YYYYMMDD-SEQ 格式 ID（domain 层统一生成器）
///
/// 供 CLI 和管线共同使用。SEQ 从已有 Outcome 列表推导，
/// 按日期分组后取当天最大 SEQ + 1。
pub fn generate_outcome_id(existing: &[Outcome], date: &str) -> String {
    let max_seq = existing.iter()
        .filter_map(|o| o.id.strip_prefix(&format!("OUT-{}", date)))
        .filter_map(|s| s.strip_prefix('-'))
        .filter_map(|s| s.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("OUT-{}-{:03}", date, max_seq + 1)
}

/// 结果判定 — 判断 vs 现实
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OutcomeVerdict {
    /// 判断被证实
    Confirmed,
    /// 判断部分正确（方向对，幅度/范围偏差）
    PartiallyConfirmed,
    /// 判断被证伪
    #[serde(alias = "Refuted")]
    Invalidated,
    /// 尚无法判断
    #[serde(alias = "Inconclusive")]
    Unknown,
}

/// 结果影响程度
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ImpactLevel {
    Low,
    Medium,
    High,
}

impl ImpactLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImpactLevel::Low => "low",
            ImpactLevel::Medium => "medium",
            ImpactLevel::High => "high",
        }
    }
}

/// 结果记录：决策的验证
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outcome {
    /// 唯一 ID（OUT-YYYYMMDD-SEQ）
    pub id: String,
    /// 关联的 DEC-XXXX
    pub decision_id: String,
    /// 关联的 Thesis ID（从 decision 反查）
    pub thesis_id: String,
    /// 观察到的证据/结果描述
    pub description: String,
    /// 结果判定
    #[serde(alias = "result")]
    pub verdict: OutcomeVerdict,
    /// 影响程度
    pub impact: ImpactLevel,
    /// 记录日期
    #[serde(alias = "recorded_at")]
    pub date: String,
    /// 触发此判定的证据 ID 列表
    #[serde(default)]
    pub supporting_evidence: Vec<String>,
    /// 当初判断时期望的信号方向（归因模型: 预期）
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub expected_signal: String,
    /// 实际观察到的情况（归因模型: 实际）
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub actual_signal: String,
    /// 期望 vs 实际的偏差（归因模型: delta，一句话概括）
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub delta: String,
}

impl Outcome {
    /// 创建新的 Outcome 记录（同时产出审计事件）
    ///
    /// 审计事件包含 verdict、decision_id、impact 三个字段供事件过滤。
    pub fn new(
        id: String,
        decision_id: String,
        thesis_id: String,
        description: String,
        verdict: OutcomeVerdict,
        impact: ImpactLevel,
        date: String,
    ) -> (Self, ObjectEvent) {
        let record = Self {
            id: id.clone(),
            decision_id: decision_id.clone(),
            thesis_id: thesis_id.clone(),
            description,
            verdict,
            impact,
            date,
            supporting_evidence: vec![],
            expected_signal: String::new(),
            actual_signal: String::new(),
            delta: String::new(),
        };
        let event = ObjectEvent::new(
            ObjectEventType::OutcomeRecorded,
            &record.id,
            "outcome",
            serde_json::json!({
                "verdict": format!("{:?}", record.verdict),
                "decision_id": decision_id,
                "thesis_id": thesis_id,
                "impact": record.impact.as_str(),
            }),
            "sulix_outcome_cli",
        );
        (record, event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_log::OBJECT_EVENT_SCHEMA_VERSION;

    #[test]
    fn test_generate_outcome_id_first_of_day() {
        let existing = vec![];
        let id = generate_outcome_id(&existing, "2026-07-09");
        assert_eq!(id, "OUT-2026-07-09-001");
    }

    #[test]
    fn test_generate_outcome_id_increments_seq() {
        let existing = vec![
            Outcome {
                id: "OUT-2026-07-09-001".into(), decision_id: "DEC-001".into(),
                thesis_id: "t1".into(), description: "test".into(),
                verdict: OutcomeVerdict::Confirmed, impact: ImpactLevel::Medium,
                date: "2026-07-09".into(), supporting_evidence: vec![],
                expected_signal: String::new(), actual_signal: String::new(),
                delta: String::new(),
            },
            Outcome {
                id: "OUT-2026-07-09-002".into(), decision_id: "DEC-002".into(),
                thesis_id: "t2".into(), description: "test".into(),
                verdict: OutcomeVerdict::PartiallyConfirmed, impact: ImpactLevel::Low,
                date: "2026-07-09".into(), supporting_evidence: vec![],
                expected_signal: String::new(), actual_signal: String::new(),
                delta: String::new(),
            },
        ];
        let id = generate_outcome_id(&existing, "2026-07-09");
        assert_eq!(id, "OUT-2026-07-09-003");
    }

    #[test]
    fn test_generate_outcome_id_ignores_other_dates() {
        let existing = vec![
            Outcome {
                id: "OUT-2026-07-09-005".into(), decision_id: "DEC-001".into(),
                thesis_id: "t1".into(), description: "test".into(),
                verdict: OutcomeVerdict::Confirmed, impact: ImpactLevel::Medium,
                date: "2026-07-09".into(), supporting_evidence: vec![],
                expected_signal: String::new(), actual_signal: String::new(),
                delta: String::new(),
            },
        ];
        // Different date should not see existing
        let id = generate_outcome_id(&existing, "2026-07-10");
        assert_eq!(id, "OUT-2026-07-10-001");
    }

    #[test]
    fn test_outcome_new_creates_event() {
        let (o, evt) = Outcome::new(
            "OUT-20260708-001".into(),
            "DEC-001".into(),
            "thesis-1".into(),
            "用户认可方向".into(),
            OutcomeVerdict::PartiallyConfirmed,
            ImpactLevel::Medium,
            "2026-07-08".into(),
        );

        assert_eq!(o.id, "OUT-20260708-001");
        assert_eq!(o.decision_id, "DEC-001");
        assert_eq!(o.thesis_id, "thesis-1");
        assert_eq!(o.verdict, OutcomeVerdict::PartiallyConfirmed);
        assert_eq!(o.impact, ImpactLevel::Medium);

        assert_eq!(evt.event_type, ObjectEventType::OutcomeRecorded);
        assert_eq!(evt.object_id, "OUT-20260708-001");
        assert_eq!(evt.summary["verdict"], "PartiallyConfirmed");
        assert_eq!(evt.summary["decision_id"], "DEC-001");
    }
}
