//! 领域模型层
//!
//! 统一存放全系统的核心领域类型，避免循环依赖。
//!
//! 子模块：
//! - theme: 主题与主题分析
//! - evidence: 证据与事实基础
//! - thesis: 论题与信念声明

pub mod theme;
pub mod evidence;
pub mod thesis;

// ===== 统一重导出 =====
// 调用方可以直接 `use crate::domain::{Theme, Thesis, Evidence, ...}`

#[allow(unused_imports)]
pub use theme::*;
#[allow(unused_imports)]
pub use evidence::*;
#[allow(unused_imports)]
pub use thesis::*;
