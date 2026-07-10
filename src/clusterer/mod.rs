//! Clusterer — 主题聚类与分析（重导出入口）
//!
//! 从 836 行缩减为重导出入口。
//! 子模块：clustering / synthesis / llm_prededup
//!
//! 所有共享类型已迁移至 `crate::domain`，此处通过子模块重导向后兼容。

// ===== 子模块声明 =====

pub mod clustering;
pub mod llm_prededup;
pub mod synthesis;

// ===== Explicit re-exports (was wildcard `pub use clustering::*` — narrowed for API clarity) =====

pub use clustering::cluster_articles;
pub use llm_prededup::llm_prededup;
pub use synthesis::synthesize;
