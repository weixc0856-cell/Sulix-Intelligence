//! 领域模型层
//!
//! 统一存放全系统的核心领域类型，避免循环依赖。
//!
//! 子模块：
//! - theme:     主题与主题分析
//! - evidence:  证据与事实基础
//! - thesis:    论题与信念声明
//! - observation: 原始观察（认知链路第一环）
//! - action:      行动建议（决策支持输出）
//! - outcome:     结果验证（判断 vs 现实）
//! - reflection:  反思复盘（为什么对/错）

pub mod action;
pub mod strategic_domain;
pub mod revision;
pub mod evidence;
pub mod investigation;
pub mod outcome;
pub mod premium;
pub mod reflection;
pub mod theme;
pub mod decision;
pub mod editor_note;
pub mod thesis;
pub mod artifact;
pub mod localized;

// ===== 统一重导出 =====
// 显式列出的类型 — 只有外部实际消费的类型才暴露。
// 避免通配符 pub use submodule::*，防止隐藏 dead code 和 API surface 膨胀。

pub use action::{DecisionHorizon, DecisionStability, DecisionType};
pub use decision::{DecisionRecord, DecisionState, DecisionTransition, ThesisDecision};
pub use strategic_domain::StrategicDomain;
pub use revision::{build_revision_history, Revision};
pub use editor_note::EditorNote;
pub use evidence::{compute_confidence, Evidence, FactBaseEntry, Stance};
pub use investigation::{Investigation, InvestigationReport, Question, QuestionStatus};
pub use outcome::{generate_outcome_id, Outcome, OutcomeVerdict};
pub use premium::{PremiumReport, SpecialTopic};
pub use reflection::Reflection;
pub use localized::Localized;
pub use theme::{AdverseScenario, Assumption, CausalChain, Summary, Theme, ThemeAnalysis};
pub use thesis::{
    ConfidenceSnapshot, ConfidenceTrigger, LifecycleEvent, LifecycleEventKind, StatusTransition,
    Thesis, ThesisRepository, ThesisStatus, TransitionTrigger,
};
