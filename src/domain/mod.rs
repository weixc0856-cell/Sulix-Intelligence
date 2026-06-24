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

pub mod theme;
pub mod evidence;
pub mod thesis;
pub mod observation;
pub mod action;
pub mod outcome;
pub mod reflection;

// ===== 统一重导出 =====
// 调用方可以直接 `use crate::domain::{Theme, Thesis, Evidence, ...}`

#[allow(unused_imports)]
pub use theme::*;
#[allow(unused_imports)]
pub use evidence::*;
#[allow(unused_imports)]
pub use thesis::*;
#[allow(unused_imports)]
pub use observation::*;
#[allow(unused_imports)]
pub use action::*;
#[allow(unused_imports)]
pub use outcome::*;
#[allow(unused_imports)]
pub use reflection::*;
