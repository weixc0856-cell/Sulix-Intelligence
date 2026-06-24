//! Clusterer — 主题聚类与分析（重导出入口）
//!
//! 从 836 行缩减为重导出入口。
//! 子模块：clustering / analysis / synthesis / llm_prededup
//!
//! 共享类型定义已迁移至 `crate::domain`，此处保持向后兼容重导出。

// ===== 共享类型（由 domain 层提供）=====

#[allow(unused_imports)]
pub use crate::domain::theme::{
    AdverseScenario, Assumption, CausalChain, Summary, Theme, ThemeAnalysis,
};
pub use crate::domain::evidence::FactBaseEntry;

// ===== 子模块声明 =====

pub mod analysis;
pub mod clustering;
pub mod llm_prededup;
pub mod synthesis;

// ===== 重导（向后兼容）=====

pub use analysis::*;
pub use clustering::*;
pub use llm_prededup::*;
pub use synthesis::*;

// 保持 change_detection → hermes 重导向后兼容
pub use crate::hermes::{detect_changes_llm, detect_changes_rule, ChangeSummary};
