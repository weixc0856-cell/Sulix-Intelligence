//! Artifact 模块 — manifest/report/builder
//!
//! manifest 是存储快照，由 delivery::publisher 在验证门后生成。
//! report 是 PipelineReport 增强，save() 双写 data/ 和 public/。
//! builder 是 publishing 内部调用的纯函数。

pub mod builder;
pub mod manifest;
pub mod report;
