//! PipelineStep trait — 统一管线步骤抽象
//!
//! 参考 ripgrep `Matcher` trait 的设计：
//! - 2 个必需方法：name(), run()
//! - run() 是核心方法，类比 Matcher::find_at()
//! - 步骤通过泛型约束 `PipelineStep<I, O>` 保证输入输出类型安全
//!
//! 架构类比：
//!   PipelineStep = Matcher（包含核心逻辑）
//!   PipelineDriver = Searcher（驱动循环）
//!   StepContext = 共享上下文（类似 ripgrep 的 Haystack）
//!
//! 管线固定 3 步，编译期类型安全（无 type erasure）：

use std::fmt::Debug;

use crate::context::StepContext;

/// 统一管线步骤 trait
///
/// # 类型参数
/// - `I`: 输入类型（Observation / Signal / Thesis）
/// - `O`: 输出类型（Signal / Thesis / Decision）
///
/// # 设计原则
/// - 仅必需：name() 用于日志，run() 是核心处理方法
/// - 不要求 Clone（DecisionMappingStep 不需要 Clone）
/// - 所有步骤 Send + Sync（支持异步和未来并行）
pub trait PipelineStep<I, O>: Send + Sync {
    /// 步骤名称（日志/调试用）
    fn name(&self) -> &'static str;

    /// 核心处理方法 — 类比 ripgrep Matcher::find_at()
    ///
    /// 输入一批数据，产生 0..N 条输出。
    /// 错误向上传播，由 PipelineDriver 或调用方处理。
    fn run(
        &self,
        input: Vec<I>,
        ctx: &StepContext,
    ) -> impl std::future::Future<Output = anyhow::Result<Vec<O>>> + Send;

    /// 步骤执行统计（可选覆盖）
    fn stats(&self) -> StepStats {
        StepStats::default()
    }
}

/// 步骤执行统计
#[derive(Debug, Clone, Default)]
pub struct StepStats {
    /// 步骤名称
    pub step_name: &'static str,
    /// 输入数量
    pub items_in: usize,
    /// 输出数量
    pub items_out: usize,
    /// LLM 调用次数
    pub llm_calls: u64,
    /// Fast Path 处理数量
    pub fast_path_count: usize,
    /// Slow Path 处理数量
    pub slow_path_count: usize,
}

/// 管线运行统计
#[derive(Debug, Clone, Default)]
pub struct PipelineStats {
    /// 各步骤统计
    pub step_stats: Vec<StepStats>,
    /// 开始时间
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// 结束时间
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl PipelineStats {
    /// 创建新的管线统计
    pub fn new() -> Self {
        Self {
            step_stats: Vec::new(),
            started_at: chrono::Utc::now(),
            finished_at: None,
        }
    }

    /// 完成管线（记录结束时间）
    pub fn finish(&mut self) {
        self.finished_at = Some(chrono::Utc::now());
    }

    /// 运行耗时（毫秒）
    pub fn elapsed_ms(&self) -> i64 {
        let end = self.finished_at.unwrap_or_else(chrono::Utc::now);
        (end - self.started_at).num_milliseconds()
    }
}
