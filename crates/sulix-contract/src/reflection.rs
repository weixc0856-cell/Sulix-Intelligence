//! Reflection — 偏差分析 + 信念更新
//!
//! Reflection 是系统的"元认知"：
//!   1. 我过去判断了什么？
//!   2. 实际结果是什么？
//!   3. 偏差在哪里？
//!   4. 我学到了什么？（信念更新）
//!
//! 这是系统真正"成长"的方式 — 不是积累更多新闻，
//! 而是不断修正对世界的假设。
//!
//! 契约边界：
//!   Producer: Memory (自动触发或手动触发)
//!   Consumer: Memory (Belief Update 的输入)
//!             Renderer (展示反思看板)

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 偏差分析 + 信念更新
///
/// # 设计原则
/// - 每条 Reflection 绑定到一个 Thesis + Outcome
/// - lesson 是真正可迁移的经验
/// - belief_update 指向 Belief 的修改建议
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Reflection {
    /// 唯一 ID，格式 "ref_xxx"
    pub id: String,

    /// 关联的 Thesis ID
    pub thesis_id: String,

    /// 原始判断
    pub previous_claim: String,

    /// 预期结果
    pub expected_outcome: String,

    /// 实际结果
    pub actual_outcome: String,

    /// 结果判定
    pub verdict: OutcomeVerdict,

    /// 偏差分析
    pub deviation_analysis: String,

    /// 核心教训
    pub lesson: String,

    /// 信念更新建议
    #[serde(default)]
    pub belief_update: Option<String>,

    /// 创建日期（ISO 8601）
    pub created_at: String,
}

/// 结果判定
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum OutcomeVerdict {
    /// 预期正确
    Confirmed,
    /// 预期错误
    Invalidated,
    /// 部分正确（结果在预期范围内，但偏差较大）
    Partial,
    /// 暂时无法判定（仍在进行中）
    Pending,
}
