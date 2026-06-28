//! Analysis — 主题分析与 SVI 计算（子模块入口）
//!
//! 子模块：
//! - analyzer: analyze_theme + challenge_theme (主题分析编排)
//! - svi: calculate_svi + map_to_scl (SVI 评分)
//! - causal: parse_causal_chain (因果链解析)

pub mod analyzer;
pub mod asi;
pub mod causal;
pub mod svi;

// Explicit re-exports — avoid wildcards to keep API surface clear
pub use analyzer::{analyze_theme, challenge_theme};
// asi types accessed via crate::engine::analysis::asi::* directly — no wildcard needed
// (parse_causal_chain is pub(super) in causal.rs — no items to re-export)
pub use svi::calculate_svi;
