//! 结果领域模型
//!
//! Outcome 记录 Thesis 的预测结果——"我的判断对了吗？"
//! 这是认知链路从"判断"到"结果验证"的关键闭环。
//!
//! 认知链路定位：
//!   ... → Decision（决策）→ Outcome（结果）→ Reflection（复盘）
//!                            ↑
//!                      Outcome 在此：判断的验证
//!
//! Outcome 是 Meta Layer 的基础：没有 Outcome，就无法知道判断是否正确。
//! 没有正确率（Historical Accuracy）数据，Reflection 就是空谈。
//!
//! v2 变更：
//!   - OutcomeType → OutcomeVerdict（语义更精确）
//!   - Refuted → Invalidated, Inconclusive → Unknown
//!   - 增加 description / supporting_evidence
//!   - 移除 expected / actual / deviation_analysis（过于细粒度）
//!   - serde aliases 确保旧数据向后兼容

use serde::{Deserialize, Serialize};
use crate::event_log::{ObjectEvent, ObjectEventType};

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

/// 结果记录：判断的验证
///
/// v3: 添加最小归因模型（§4.2）。让 Outcome 从"记录结果"升级为"学习和归因结果"。
///
/// 归因字段：
///   - expected_signal: 当初判断时期望的信号方向
///   - actual_signal: 实际观察到的情况
///   - delta: 期望 vs 实际的偏差（一句话概括）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outcome {
    /// 唯一 ID
    pub id: String,
    /// 关联的 Thesis ID
    pub thesis_id: String,
    /// 结果描述（概括判断 vs 现实）
    pub description: String,
    /// 结果判定
    #[serde(alias = "result")]
    pub verdict: OutcomeVerdict,
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
    /// 简化版构造函数：只填核心字段，supporting_evidence/expected/actual/delta 默认空
    pub fn new(
        id: String,
        thesis_id: String,
        description: String,
        verdict: OutcomeVerdict,
        date: String,
    ) -> (Self, ObjectEvent) {
        let record = Self {
            id: id.clone(),
            thesis_id: thesis_id.clone(),
            description,
            verdict,
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
                "thesis_id": thesis_id,
            }),
            "agent_publish",
        );
        (record, event)
    }
}
