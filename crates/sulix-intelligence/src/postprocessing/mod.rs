//! Post-Processing — 管线输出后的辅助生成步骤
//!
//! 这些步骤**不是** IntelligencePipeline 的一部分。
//! 它们在 `pipeline.run()` 之后独立调用，输入 IntelligenceOutput，
//! 输出文本/摘要供 MDX renderer 消费。
//!
//! 设计原则：
//! - 不产生新的结构化契约（不需要新的 contract::* 类型）
//! - 可以失败（返回空字符串/空 Vec），不影响管线主流程
//! - 与旧 publishing::generate 的职责一一对应

pub mod calibration;
pub mod editor_note;
pub mod summary;

pub use calibration::calibrate;
pub use editor_note::analyze_personal_impact;
pub use summary::synthesize;
