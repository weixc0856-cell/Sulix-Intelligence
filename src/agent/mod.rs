//! Agent 模块 — 多 Agent 协作系统
//!
//! - calibration: 认知校准（扎心问题）
//! - decay:       记忆墓地维护
//! - editor:      幕僚长分析（个人影响）
//! - scan:        信号初筛（Gate v1.1）
//! - signal:      源抓取/去重/丰富/实体提取（从 main.rs 迁入）
//! - research:    分流/聚类/分析/蓝军/认知引擎（从 main.rs 迁入）

pub mod calibration;
pub mod decay;
pub mod editor;
pub mod scan;
pub mod signal;
pub mod research;
