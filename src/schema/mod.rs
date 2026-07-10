//! Schema 验证模块 — Engine→R2 之间的 Contract Enforcement Layer
//!
//! 在 Object Creation（publishing.rs）和 R2 Upload（main.rs）之间执行：
//!   1. 必填字段非空
//!   2. Evidence 数组非空（Phase 0 警告，未来拒绝）
//!   3. Confidence 范围 [0,1]
//!   4. 对象版本化
//!
//! 未通过验证的对象**不上传 R2**，写入 validation_report.json。

pub mod assessment;
pub mod decision;
pub mod mapper;
pub mod signal;
pub mod validator;

pub use validator::ValidationReport;
