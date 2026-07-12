//! DecisionMappingStep — Thesis → Decision 映射
//!
//! 规则约束 + LLM 推理的混合决策系统。
//!
//! 架构（类比飞行控制系统）:
//!   - RuleEngine = 飞控系统（确保不超出安全包线）
//!   - LLM Judge = 飞行员（提供具体判断和推理）
//!
//! 双路径设计（类比 ripgrep Fast/Slow Path）:
//!   - Fast Path:  规则映射 + 平滑，零 LLM 调用
//!   - Slow Path:  规则映射 → LLM Judge 深度推理
//!   - Auto 选择:  根据 thesis.confidence + evidence 数量自动选择
//!
//! 规则层（来自 `engine/decision.rs` 的完整映射）:
//!   - ThesisStatus → DecisionType 确定性映射
//!   - Decision Smoothing (2-day hysteresis)
//!   - Stability 计算 (3-day consensus)
//!
//! 映射规则:
//!   Proposed    → Learn (180d)
//!   Active      → Monitor (90d)
//!   Strengthening → Build (90d)
//!   Weakening   → Learn (30d)
//!   Dormant     → Ignore (180d)
//!   Confirmed   → Monitor (180d)
//!   Invalidated → Exit (Immediate)

use anyhow::Result;

use sulix_config::LlmConfig;
use sulix_contract as contract;
use sulix_contract::ThesisStatus;

use super::context::StepContext;
use super::step::PipelineStep;

/// 规则引擎裁决
#[derive(Debug, Clone)]
pub struct RuleVerdict {
    pub passed: bool,
    pub reasoning: String,
    pub suggested_action: Option<String>,
}

/// 规则引擎 — 约束层（纯确定规则）
pub struct RuleEngine;

impl RuleEngine {
    pub fn map_thesis(&self, thesis: &contract::Thesis) -> StatusMapping {
        let claim = &thesis.claim;
        let evidence_count = thesis.evidence.len();
        let (raw_type, horizon, rationale, confidence) = match &thesis.status {
            ThesisStatus::Proposed => (
                contract::DecisionType::Learn,
                contract::DecisionHorizon::Days180,
                format!("新判断 '{}' — 需要更多证据", claim),
                thesis.confidence.min(0.5),
            ),
            ThesisStatus::Active | ThesisStatus::Pending => {
                if evidence_count >= 3 {
                    (
                        contract::DecisionType::Monitor,
                        contract::DecisionHorizon::Days90,
                        format!("'{}' 有 {} 条证据支持 — 值得关注", claim, evidence_count),
                        thesis.confidence,
                    )
                } else {
                    (
                        contract::DecisionType::Monitor,
                        contract::DecisionHorizon::Days30,
                        format!("'{}' 仅 {} 条证据 — 继续收集", claim, evidence_count),
                        thesis.confidence.max(0.4),
                    )
                }
            }
            ThesisStatus::Strengthening => (
                contract::DecisionType::Build,
                contract::DecisionHorizon::Days90,
                format!("'{}' 正在强化 — 建议投入资源", claim),
                thesis.confidence.max(0.6),
            ),
            ThesisStatus::Weakening => (
                contract::DecisionType::Learn,
                contract::DecisionHorizon::Days30,
                format!("'{}' 正在弱化 — 需要重新评估", claim),
                thesis.confidence.min(0.5),
            ),
            ThesisStatus::Confirmed => (
                contract::DecisionType::Monitor,
                contract::DecisionHorizon::Days180,
                format!("'{}' 已被验证 — 跟踪衍生影响", claim),
                thesis.confidence.max(0.7),
            ),
            ThesisStatus::Invalidated => (
                contract::DecisionType::Exit,
                contract::DecisionHorizon::Immediate,
                format!("'{}' 已被证伪 — 立即退出", claim),
                0.0,
            ),
        };
        StatusMapping {
            decision_type: raw_type,
            horizon,
            rationale,
            confidence,
        }
    }

    pub fn smooth(
        &self,
        new_action: &contract::DecisionType,
        last_action: Option<&contract::DecisionType>,
    ) -> contract::DecisionType {
        match last_action {
            None => new_action.clone(),
            Some(last) => {
                if matches!(new_action, contract::DecisionType::Exit) || new_action == last {
                    new_action.clone()
                } else {
                    log::info!(
                        "  🛑 Decision Smoothing: 抑制 {:?} → {:?}",
                        last,
                        new_action
                    );
                    last.clone()
                }
            }
        }
    }

    pub fn stability(&self, action: &contract::DecisionType, history_count: usize) -> String {
        if matches!(action, contract::DecisionType::Exit) {
            return "Final".into();
        }
        if history_count >= 3 {
            "Stable".into()
        } else {
            "Volatile".into()
        }
    }
}

/// Thesis 状态映射结果
#[derive(Debug, Clone)]
pub struct StatusMapping {
    pub decision_type: contract::DecisionType,
    pub horizon: contract::DecisionHorizon,
    pub rationale: String,
    pub confidence: f64,
}

/// 处理路径选择 — 类比 ripgrep Fast Path 检查
///
/// ripgrep 的 `is_line_by_line_fast()` 检查 matcher 是否支持 fast path。
/// Sulix 的双路径选择逻辑：
///   - Fast Path:  纯规则，零 LLM 调用
///   - Slow Path:  规则 + LLM 深度推理
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcessingPath {
    Fast,
    Slow,
}

impl ProcessingPath {
    pub fn auto_select(thesis: &contract::Thesis) -> Self {
        let is_terminal = matches!(
            thesis.status,
            contract::ThesisStatus::Confirmed | contract::ThesisStatus::Invalidated
        );
        let is_low_value = thesis.confidence < 0.5 || thesis.evidence.is_empty();
        if is_terminal || is_low_value {
            Self::Fast
        } else {
            Self::Slow
        }
    }
    pub fn needs_llm(&self) -> bool {
        matches!(self, Self::Slow)
    }
}

/// LLM Judge — 推理层
struct LlmJudge {
    llm_config: LlmConfig,
    api_key: String,
}

impl LlmJudge {
    async fn judge(&self, thesis: &contract::Thesis, _ctx: &StepContext) -> Result<String> {
        let client = sulix_llm::create_client(120)?;
        let system_prompt = r#"You are a strategic decision analyst. Given a thesis (a falsifiable claim about the future), assess:
1. Evidence strength — does the evidence genuinely support the claim?
2. Counter-arguments — what would disprove this?
3. Decision fit — is the suggested action type appropriate?

Output strict JSON:
{"evidence_assessment": "<brief assessment>", "counter_args": "<key counter-argument>", "decision_fit": true/false}"#;
        let user_prompt = format!(
            "判断: {} | 置信度: {:.2} | 状态: {:?} | 证据: {}",
            thesis.claim,
            thesis.confidence,
            thesis.status,
            thesis.evidence.len()
        );
        let raw = sulix_llm::call_with_retry_raw(
            &client,
            &self.api_key,
            &self.llm_config,
            system_prompt,
            &user_prompt,
        )
        .await?;
        let parsed = sulix_llm::parse_json_lenient(&raw);
        match parsed {
            Ok(json) => Ok(format!(
                "LLM 评估: {}",
                json["evidence_assessment"].as_str().unwrap_or(&raw)
            )),
            Err(_) => Ok(raw.chars().take(200).collect()),
        }
    }
}

/// Decision Mapping 步骤
pub struct DecisionMappingStep {
    rule_engine: RuleEngine,
    llm_judge: Option<LlmJudge>,
    last_decisions: Vec<contract::Decision>,
}

impl DecisionMappingStep {
    /// 获取最后决策列表
    pub fn last_decisions(&self) -> &[contract::Decision] {
        &self.last_decisions
    }

    /// 获取 RuleEngine 引用
    pub fn rule_engine(&self) -> &RuleEngine {
        &self.rule_engine
    }

    fn find_last_decision(&self, thesis_id: &str) -> Option<&contract::Decision> {
        self.last_decisions
            .iter()
            .find(|d| d.thesis_id == thesis_id)
    }

    pub async fn map(
        &self,
        theses: Vec<contract::Thesis>,
        ctx: &StepContext,
    ) -> Result<Vec<contract::Decision>> {
        if theses.is_empty() {
            return Ok(vec![]);
        }
        log::info!("⚖️ DecisionMapping: {} theses", theses.len());
        let mut decisions = Vec::new();

        for thesis in &theses {
            let mapping = self.rule_engine.map_thesis(thesis);
            let last = self.find_last_decision(&thesis.id);
            let smoothed_action = self
                .rule_engine
                .smooth(&mapping.decision_type, last.map(|d| &d.action));

            // Dual path: Fast (no LLM) / Slow (LLM Judge)
            let path = ProcessingPath::auto_select(thesis);
            let reasoning = if path.needs_llm() {
                if let Some(ref judge) = self.llm_judge {
                    match judge.judge(thesis, ctx).await {
                        Ok(llm_r) => format!("{}\n{}", mapping.rationale, llm_r),
                        Err(_) => mapping.rationale.clone(),
                    }
                } else {
                    mapping.rationale.clone()
                }
            } else {
                mapping.rationale.clone()
            };

            let horizon = if let contract::DecisionType::Exit = smoothed_action {
                contract::DecisionHorizon::Immediate
            } else {
                mapping.horizon
            };
            let ts = chrono::Utc::now().format("%Y%m%d%H%M%S");
            let decision = contract::Decision {
                id: format!("dec_{}_{:04}", ts, decisions.len() + 1),
                thesis_id: thesis.id.clone(),
                action: smoothed_action,
                confidence: mapping.confidence,
                horizon,
                reasoning,
                made_at: ctx.today.clone(),
                rule_passed: true,
                requires_review: thesis.evidence.is_empty() || thesis.confidence < 0.3,
                review_reason: if thesis.evidence.is_empty() {
                    Some("无证据".into())
                } else if thesis.confidence < 0.3 {
                    Some(format!("低置信度 {:.2}", thesis.confidence))
                } else {
                    None
                },
            };
            decisions.push(decision);
        }

        if ctx.should_write_debug() {
            let artifact = super::Artifact::Decisions(decisions.clone());
            let json = artifact.to_json()?;
            if let Some(dir) = &ctx.debug_dir {
                let path = dir.join(format!("{}.decision.output.json", ctx.today));
                std::fs::create_dir_all(dir)?;
                std::fs::write(&path, &json)?;
            }
        }
        log::info!("✅ DecisionMapping: {} decisions", decisions.len());
        Ok(decisions)
    }
}

// ===== DecisionMappingStepBuilder =====

/// DecisionMappingStep 构建器
#[derive(Default)]
pub struct DecisionMappingStepBuilder {
    llm_judge: Option<LlmJudge>,
    last_decisions: Vec<contract::Decision>,
}

impl DecisionMappingStepBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_llm_judge(mut self, llm_config: LlmConfig, api_key: &str) -> Self {
        self.llm_judge = Some(LlmJudge {
            llm_config,
            api_key: api_key.to_string(),
        });
        self
    }

    pub fn with_last_decisions(mut self, decisions: Vec<contract::Decision>) -> Self {
        self.last_decisions = decisions;
        self
    }

    pub fn build(self) -> DecisionMappingStep {
        DecisionMappingStep {
            rule_engine: RuleEngine,
            llm_judge: self.llm_judge,
            last_decisions: self.last_decisions,
        }
    }
}

// ===== PipelineStep trait 实现 =====

impl PipelineStep<contract::Thesis, contract::Decision> for DecisionMappingStep {
    fn name(&self) -> &'static str {
        "DecisionMapping"
    }

    async fn run(
        &self,
        input: Vec<contract::Thesis>,
        ctx: &StepContext,
    ) -> anyhow::Result<Vec<contract::Decision>> {
        self.map(input, ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_thesis(
        status: ThesisStatus,
        confidence: f64,
        evidence_count: usize,
    ) -> contract::Thesis {
        contract::Thesis {
            id: "thesis_test".into(),
            claim: "Test thesis".into(),
            confidence,
            evidence: (0..evidence_count).map(|i| format!("sig_{}", i)).collect(),
            status,
            falsification_conditions: vec![],
            time_horizon: "12_months".into(),
            theme: None,
            belief_statement: None,
        }
    }

    #[test]
    fn test_proposed_maps_to_learn() {
        let engine = RuleEngine;
        let m = engine.map_thesis(&make_thesis(ThesisStatus::Proposed, 0.5, 1));
        assert!(matches!(m.decision_type, contract::DecisionType::Learn));
        assert!(matches!(m.horizon, contract::DecisionHorizon::Days180));
    }

    #[test]
    fn test_strengthening_maps_to_build() {
        let engine = RuleEngine;
        let m = engine.map_thesis(&make_thesis(ThesisStatus::Strengthening, 0.7, 5));
        assert!(matches!(m.decision_type, contract::DecisionType::Build));
    }

    #[test]
    fn test_weakening_maps_to_learn() {
        let engine = RuleEngine;
        let m = engine.map_thesis(&make_thesis(ThesisStatus::Weakening, 0.4, 2));
        assert!(matches!(m.decision_type, contract::DecisionType::Learn));
    }

    #[test]
    fn test_invalidated_maps_to_exit() {
        let engine = RuleEngine;
        let m = engine.map_thesis(&make_thesis(ThesisStatus::Invalidated, 0.0, 0));
        assert!(matches!(m.decision_type, contract::DecisionType::Exit));
        assert!(matches!(m.horizon, contract::DecisionHorizon::Immediate));
    }

    #[test]
    fn test_confirmed_maps_to_monitor() {
        let engine = RuleEngine;
        let m = engine.map_thesis(&make_thesis(ThesisStatus::Confirmed, 0.8, 5));
        assert!(matches!(m.decision_type, contract::DecisionType::Monitor));
    }

    #[test]
    fn test_smooth_exit_always_immediate() {
        assert!(matches!(
            RuleEngine.smooth(
                &contract::DecisionType::Exit,
                Some(&contract::DecisionType::Monitor)
            ),
            contract::DecisionType::Exit
        ));
    }

    #[test]
    fn test_smooth_same_action_continues() {
        let a = contract::DecisionType::Build;
        assert_eq!(RuleEngine.smooth(&a, Some(&a)), a);
    }

    #[test]
    fn test_smooth_different_action_suppressed() {
        let r = RuleEngine.smooth(
            &contract::DecisionType::Build,
            Some(&contract::DecisionType::Monitor),
        );
        assert!(matches!(r, contract::DecisionType::Monitor));
    }

    #[test]
    fn test_stability_exit_is_final() {
        assert_eq!(
            RuleEngine.stability(&contract::DecisionType::Exit, 0),
            "Final"
        );
    }
    #[test]
    fn test_stability_three_days_stable() {
        assert_eq!(
            RuleEngine.stability(&contract::DecisionType::Monitor, 3),
            "Stable"
        );
    }
    #[test]
    fn test_stability_less_than_three_volatile() {
        assert_eq!(
            RuleEngine.stability(&contract::DecisionType::Monitor, 1),
            "Volatile"
        );
    }

    // ===== ProcessingPath tests =====

    fn make_path_thesis(
        confidence: f64,
        evidence: usize,
        status: ThesisStatus,
    ) -> contract::Thesis {
        make_thesis(status, confidence, evidence)
    }

    #[test]
    fn test_processing_path_high_confidence_is_slow() {
        let t = make_path_thesis(0.7, 3, ThesisStatus::Active);
        assert_eq!(ProcessingPath::auto_select(&t), ProcessingPath::Slow);
    }

    #[test]
    fn test_processing_path_low_confidence_is_fast() {
        let t = make_path_thesis(0.3, 2, ThesisStatus::Active);
        assert_eq!(ProcessingPath::auto_select(&t), ProcessingPath::Fast);
    }

    #[test]
    fn test_processing_path_no_evidence_is_fast() {
        let t = make_path_thesis(0.6, 0, ThesisStatus::Active);
        assert_eq!(ProcessingPath::auto_select(&t), ProcessingPath::Fast);
    }

    #[test]
    fn test_processing_path_invalidated_is_fast() {
        let t = make_path_thesis(0.0, 0, ThesisStatus::Invalidated);
        assert_eq!(ProcessingPath::auto_select(&t), ProcessingPath::Fast);
    }

    #[test]
    fn test_processing_path_confirmed_is_fast() {
        let t = make_path_thesis(0.9, 10, ThesisStatus::Confirmed);
        assert_eq!(ProcessingPath::auto_select(&t), ProcessingPath::Fast);
    }
}
