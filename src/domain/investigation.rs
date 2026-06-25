//! 调查引擎领域模型
//!
//! Investigation 是 Thesis 的结构化问题集，由 LLM 从 Thesis BLUF 一次生成。
//! 每个 Question 包含假设和证伪条件，用于指导证据收集和判断验证。
//!
//! 认知链路定位：
//!   Thesis → Investigation → Decision
//!                            ↑
//!                  Investigation 在此：将判断拆解为可验证的子问题
//!
//! v1 约束:
//!   - ≤5 questions per investigation
//!   - 1 falsification condition max per question
//!   - 生成一次，不重新生成

use serde::{Deserialize, Serialize};

/// 调查任务：Thesis 的结构化问题集
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Investigation {
    pub id: String,
    pub thesis_id: String,
    pub generated_at: String,
    pub questions: Vec<Question>,
}

/// 单个调查问题
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    /// 问题文本，如 "用户是否真的愿意为 Agent 付费？"
    pub text: String,
    /// 重要性 1-10
    pub importance: u8,
    /// 可检验的假设
    pub hypothesis: Option<String>,
    /// 证伪条件（最多 1 条）
    pub falsification: Option<String>,
    /// 当前答案状态
    pub status: QuestionStatus,
    /// 已收集的答案
    #[serde(default)]
    pub answers: Vec<Answer>,
    pub created_at: String,
    pub updated_at: String,
}

/// 问题答案状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QuestionStatus {
    /// 尚未找到证据
    Unanswered,
    /// 部分证据，但不足以回答
    PartiallyAnswered,
    /// 已有充分证据回答
    Answered,
    /// 证伪条件已触发
    Invalidated,
}

/// 针对某个问题的答案记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Answer {
    pub date: String,
    pub source: String,
    pub content: String,
    pub relevance: u8,
}
