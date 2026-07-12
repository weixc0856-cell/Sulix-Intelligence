//! IntelligencePipeline — 固定认知链路编排器
//!
//! 将 Observation → Signal → Thesis → Decision 编排为固定顺序的认知链路。
//! 不是 workflow engine，是固定 DAG。以后如果需要动态编排再改为 Vec<PipelineStep>。
//!
//! 这是 ripgrep Searcher 模式的引用：
//! - IntelligencePipeline = Searcher（驱动循环）
//! - 每个 Step = Sink（接收事件 + 产生输出）
//! - Artifact = 步骤间传递的契约数据

use anyhow::Result;

use sulix_contract as contract;

use super::artifact::Artifact;
use super::context::StepContext;
use super::decision_mapping::DecisionMappingStep;
use super::signal_classification::SignalClassificationStep;
use super::thesis_generation::ThesisGenerationStep;
use super::step::PipelineStats;

/// Intelligence Pipeline 输出
#[derive(Debug, Clone)]
pub struct IntelligenceOutput {
    /// 最终决策列表
    pub decisions: Vec<contract::Decision>,
    /// 中间生成的 Theses（供 Memory 存储）
    pub theses: Vec<contract::Thesis>,
    /// 中间生成的 Signals（供调试）
    pub signals: Vec<contract::Signal>,
    /// 管线运行统计
    pub stats: PipelineStats,
}

impl IntelligenceOutput {
    /// 是否产生了有效输出
    pub fn has_decisions(&self) -> bool {
        !self.decisions.is_empty()
    }

    /// 决策数量
    pub fn decision_count(&self) -> usize {
        self.decisions.len()
    }
}

/// Intelligence Pipeline — 固定 DAG 编排器
///
/// 执行顺序由 struct 字段定义保证（编译期检查）：
/// 1. SignalClassificationStep — Observation → Signal
/// 2. ThesisGenerationStep — Signal → Thesis
/// 3. DecisionMappingStep — Thesis → Decision
pub struct IntelligencePipeline {
    /// 信号分类步骤
    pub signal: SignalClassificationStep,
    /// Thesis 生成步骤
    pub thesis: ThesisGenerationStep,
    /// 决策映射步骤
    pub decision: DecisionMappingStep,
}

impl IntelligencePipeline {
    /// 创建新的 Intelligence Pipeline
    pub fn new(
        signal: SignalClassificationStep,
        thesis: ThesisGenerationStep,
        decision: DecisionMappingStep,
    ) -> Self {
        Self {
            signal,
            thesis,
            decision,
        }
    }

    /// 运行完整认知链路
    ///
    /// # 流程
    /// 1. Observations → SignalClassification → Signals
    /// 2. Signals → ThesisGeneration → Theses
    /// 3. Theses → DecisionMapping → Decisions
    ///
    /// # Debug 模式
    /// 当 ctx.debug = true 时，每个步骤的输出写为 JSON 文件到 ctx.debug_dir。
    pub async fn run(
        &self,
        observations: Vec<contract::Observation>,
        ctx: &StepContext,
    ) -> Result<IntelligenceOutput> {
        let mut stats = PipelineStats::new();
        let obs_count = observations.len();
        log::info!("🚀 IntelligencePipeline: {} observations", obs_count);

        // Step 1: Observation → Signal
        let signals = self.signal.classify(observations, ctx).await?;
        log::info!(
            "  Step 1/3: SignalClassification → {} signals",
            signals.len()
        );

        // Step 2: Signal → Thesis
        let theses = self.thesis.generate(signals.clone(), ctx).await?;
        log::info!(
            "  Step 2/3: ThesisGeneration → {} theses",
            theses.len()
        );

        // Step 3: Thesis → Decision
        let decisions = self.decision.map(theses.clone(), ctx).await?;
        log::info!(
            "  Step 3/3: DecisionMapping → {} decisions",
            decisions.len()
        );

        // Collect stats
        stats.step_stats.push(super::step::StepStats {
            step_name: "SignalClassification",
            items_in: obs_count,
            items_out: signals.len(),
            ..Default::default()
        });
        stats.step_stats.push(super::step::StepStats {
            step_name: "ThesisGeneration",
            items_in: signals.len(),
            items_out: theses.len(),
            ..Default::default()
        });
        stats.step_stats.push(super::step::StepStats {
            step_name: "DecisionMapping",
            items_in: theses.len(),
            items_out: decisions.len(),
            ..Default::default()
        });
        stats.finish();

        Ok(IntelligenceOutput {
            decisions,
            theses,
            signals,
            stats,
        })
    }

    /// 从 Artifact 开始运行（用于 Debug 重放）
    pub async fn run_from_artifact(
        &self,
        artifact: Artifact,
        ctx: &StepContext,
    ) -> Result<IntelligenceOutput> {
        let observations = artifact.into_observations()?;
        self.run(observations, ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal_classification::SignalClassificationStepBuilder;
    use crate::thesis_generation::ThesisGenerationStepBuilder;
    use crate::decision_mapping::DecisionMappingStepBuilder;
    use sulix_config::LlmConfig;

    /// 测试 pipeline 能处理空输入
    #[tokio::test]
    async fn test_pipeline_empty_input() {
        let llm_config = LlmConfig {
            api_key: Some("test".into()),
            provider: "test".into(),
            model: "test".into(),
            base_url: "http://test".into(),
            max_tokens: 100,
            temperature: 0.0,
            perplexity_key: None,
        };

        let pipeline = IntelligencePipeline::new(
            SignalClassificationStepBuilder::new(llm_config.clone(), "test").build(),
            ThesisGenerationStepBuilder::new(llm_config.clone(), "test").build(),
            DecisionMappingStepBuilder::new().build(),
        );

        let ctx = StepContext::new("2026-07-12");
        let output = pipeline.run(vec![], &ctx).await.unwrap();

        assert_eq!(output.signals.len(), 0);
        assert_eq!(output.theses.len(), 0);
        assert_eq!(output.decisions.len(), 0);
    }

    #[test]
    fn test_pipeline_output_has_decisions_false_when_empty() {
        let output = IntelligenceOutput {
            decisions: vec![],
            theses: vec![],
            signals: vec![],
            stats: PipelineStats::new(),
        };
        assert!(!output.has_decisions());
        assert_eq!(output.decision_count(), 0);
    }
}



