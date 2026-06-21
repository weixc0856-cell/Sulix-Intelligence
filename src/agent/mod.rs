//! Agent 模块 — 多 Agent 协作系统
//!
//! 当前实现:
//! - Phase A: Scan Agent（快速初筛）
//!
//! 未来:
//! - Phase B: Synthesis Agent (红) + Verification Agent (蓝) + Orchestrator

pub mod calibration;
pub mod decay;
pub mod orchestrator;
pub mod scan;
pub mod synthesis;
pub mod verification;
