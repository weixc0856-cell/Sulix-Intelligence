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

use crate::domain::strategic_domain::StrategicDomain;

/// 调查任务：Thesis 的结构化问题集
///
/// id 格式：INV-XXXX（由 InvestigationRegistry 分配）
/// state: "draft" | "active" | "completed" | "superseded" | "archived"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Investigation {
    pub id: String,
    pub thesis_id: String,
    pub generated_at: String,
    pub questions: Vec<Question>,
    #[serde(default = "default_inv_state")]
    pub state: String,
    /// 主战略领域
    #[serde(default)]
    pub primary_domain: StrategicDomain,
    /// 次要战略领域（跨领域问题）
    #[serde(default)]
    pub secondary_domains: Vec<StrategicDomain>,
}

fn default_inv_state() -> String {
    "active".to_string()
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

/// Investigation Report — 对一个 Thesis 的结构化调查报告
///
/// 直接从已有 Thesis 数据派生，不需要额外 LLM 调用。
/// 结构：Core Question → Supporting Evidence → Counter Evidence
///       → Key Unknowns → Falsification Conditions → Preliminary Conclusion
///
/// 输出到 output/investigation/{slug}.md
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestigationReport {
    pub thesis_id: String,
    pub thesis_title: String,
    pub date: String,
    /// 核心问题（"Will [title] hold true?"）
    pub core_question: String,
    /// 支持证据摘要（来自 thesis.evidences, stance=Supports）
    pub supporting_evidence: Vec<String>,
    /// 反对证据摘要（来自 thesis.evidences, stance=Challenges）
    pub counter_evidence: Vec<String>,
    /// 关键未知（来自承重假设证据弱的条目）
    pub key_unknowns: Vec<String>,
    /// 证伪条件（来自 thesis.falsification_conditions）
    pub falsification_conditions: Vec<String>,
    /// 初步结论（来自最新证据摘要或决策理由）
    pub preliminary_conclusion: String,
    /// 调查状态: "active" | "complete" | "archived"
    #[serde(default = "default_report_status")]
    pub status: String,
    /// 主战略领域
    #[serde(default)]
    pub primary_domain: StrategicDomain,
    /// 次要战略领域（跨领域问题）
    #[serde(default)]
    pub secondary_domains: Vec<StrategicDomain>,
}

fn default_report_status() -> String {
    "active".to_string()
}
