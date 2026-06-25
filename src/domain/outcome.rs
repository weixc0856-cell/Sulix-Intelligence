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
/// v2 精简后，只保留核心字段。description 概括偏差分析，
/// verdict 替代多字段对比。
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
}
