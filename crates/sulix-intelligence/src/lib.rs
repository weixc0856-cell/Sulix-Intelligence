//! Intelligence Pipeline — 固定认知链路
//!
//! 将 Intelligence 层拆分为 3 个独立步骤，每步通过 contract::* 类型通信。
//!
//!   Observation  →  SignalClassification  →  Signal  →  ThesisGeneration  →  Thesis  →  DecisionMapping  →  Decision
//!
//! 架构模式（引用 ripgrep）:
//! - `IntelligencePipeline` = Searcher（驱动循环）
//! - 每个 Step = Sink（接收输入 + 产生输出，不互相依赖）
//! - `Artifact` = 步骤间契约（类型安全 + JSON 可序列化）
//!
//! Debug/Production 双模式:
//! - Production: Artifact enum 内存流转，零 IO
//! - Debug: 每步将 Artifact 写为 JSON 文件到 debug_dir

pub mod artifact;
pub mod context;
pub mod decision_history;
pub mod decision_mapping;
pub mod loader;
pub mod output;
pub mod pipeline;
pub mod postprocessing;
pub mod signal_classification;
pub mod step;
pub mod thesis_generation;

pub use artifact::Artifact;
pub use context::{DebugConfig, StepContext};
pub use decision_history::{DecisionHistory, DecisionRecord};
pub use decision_mapping::{DecisionMappingStep, DecisionMappingStepBuilder};
pub use loader::{load_last_decisions, load_theses_from_memory_db, save_theses_to_memory_db};
pub use pipeline::{IntelligenceOutput, IntelligencePipeline};
pub use signal_classification::{SignalClassificationStep, SignalClassificationStepBuilder};
pub use step::{PipelineStats, PipelineStep, StepStats};
pub use thesis_generation::{ThesisGenerationStep, ThesisGenerationStepBuilder};
