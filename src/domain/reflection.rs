//! 反思领域模型
//!
//! Reflection 是认知链路的最后一环——"为什么我的判断错了/对了？"
//! 它记录的是"判断者的反思"，而非"事实的更新"。
//!
//! 认知链路定位：
//!   ... → Outcome（结果）→ Reflection（复盘）
//!                            ↑
//!                      Reflection 在此：认知链路的终点，也是下一轮的起点
//!
//! Reflection 的价值：
//!   市面上的 AI 产品都告诉你"世界发生了什么"，
//!   但没人告诉你"自己错在哪里"。
//!   Reflection 就是要回答后一个问题。

use serde::{Deserialize, Serialize};

/// 反思记录：判断的复盘
///
/// 一个完整的 Reflection 回答三个问题：
///   1. 什么错了？—— 事实、假设、还是时机？
///   2. 为什么错了？—— 错误的根因分析
///   3. 学到了什么？—— 可复用的经验教训
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reflection {
    /// 唯一 ID
    pub id: String,
    /// 关联的 Thesis ID
    pub thesis_id: String,
    /// 关联的 Outcome ID
    pub outcome_id: String,
    /// 裁决结果（对应 Astro schema verdict 字段）
    pub verdict: String,
    /// 错误/偏差原因
    pub error_reason: String,
    /// 经验教训
    #[serde(default)]
    pub lessons: Vec<String>,
    /// 创建时的置信度
    pub confidence_at_creation: f64,
    /// 现在的置信度
    pub confidence_now: f64,
    /// 反思创建日期
    pub created_at: String,
}

/// 反思摘要（供外部展示用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionSummary {
    /// 论题标题
    pub thesis_title: String,
    /// 预期 vs 实际
    pub expected: String,
    pub actual: String,
    /// 结果
    pub result: String,
    /// 核心教训
    pub key_lesson: String,
}
