//! 结果领域模型
//!
//! Outcome 记录 Thesis 的预测结果——"我的判断对了吗？"
//! 这是认知链路从"判断"到"结果验证"的关键闭环。
//!
//! 认知链路定位：
//!   ... → Action（行动）→ Outcome（结果）→ Reflection（复盘）
//!                            ↑
//!                      Outcome 在此：判断的验证
//!
//! Outcome 是 Meta Layer 的基础：没有 Outcome，就无法知道判断是否正确。
//! 没有正确率数据，Reflection 就是空谈。

use serde::{Deserialize, Serialize};

/// 结果类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OutcomeType {
    /// 判断被证实
    Confirmed,
    /// 判断部分正确
    PartiallyConfirmed,
    /// 判断被证伪
    Refuted,
    /// 尚无法判断
    Inconclusive,
}

/// 结果记录：判断的验证
///
/// 记录"我当初的判断 vs 现实结果"的偏差。
/// 这是 Reflection（复盘）的数据基础。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outcome {
    /// 唯一 ID
    pub id: String,
    /// 关联的 Thesis ID
    pub thesis_id: String,
    /// Thesis 当初的预期
    pub expected: String,
    /// 实际发生的结果
    pub actual: String,
    /// 结果类型
    pub result: OutcomeType,
    /// 记录日期
    pub recorded_at: String,
    /// 偏差分析（可选）
    #[serde(default)]
    pub deviation_analysis: Option<String>,
}
