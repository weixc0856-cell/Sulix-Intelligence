use crate::context::ContextProvider;
use crate::llm::provider::LLMProvider;
use crate::policy::{ReasoningPolicy, INSUFFICIENT_EVIDENCE_DISCLAIMER};
use crate::prompt::PromptBuilder;
use crate::reasoning::build_trace;
use crate::types::{AgentRequest, AgentResponse, AgentStage, ContextSummary, ExecutionMetadata};

/// Wall-clock ms. On wasm32 that is the Worker's `Date.now()`; on host (unit
/// tests) fall back to `std::time` so the runtime is executable off-wasm.
fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0)
    }
}

pub struct AgentRuntime {
    context: Box<dyn ContextProvider>,
    llm: Box<dyn LLMProvider>,
    prompt_builder: PromptBuilder,
}

impl AgentRuntime {
    pub fn new(context: Box<dyn ContextProvider>, llm: Box<dyn LLMProvider>) -> Self {
        Self { context, llm, prompt_builder: PromptBuilder }
    }

    pub async fn execute(&self, request: AgentRequest) -> Result<AgentResponse, String> {
        let start = now_ms();
        let now = (start / 1000.0) as i64;
        let policy = ReasoningPolicy::for_mode(&request.mode);
        let mut stages = Vec::new();

        // 1. Context
        stages.push(AgentStage::ContextBuilding);
        let ctx_result = self.context.build_context(&request.query).await?;

        // Evidence gate is fixed HERE — from the system-side context, before the
        // LLM runs. It is never recomputed from model output (answer text or a
        // future LLM-derived reasoning trace): an LLM "claiming" citations cannot
        // turn insufficient evidence into sufficient. Insufficient evidence is a
        // valid output (HTTP 200 + flag + disclaimer), not an Err.
        let evidence_count = ctx_result.context.evidence.len() as u32;
        let insufficient_evidence = policy.insufficient(evidence_count);
        let disclaimer = insufficient_evidence.then(|| INSUFFICIENT_EVIDENCE_DISCLAIMER.to_string());

        // 2. Prompt
        stages.push(AgentStage::PromptConstruction);
        let prompt = self.prompt_builder.build(&ctx_result.context, &request.mode, &request.query);

        // 3. LLM
        stages.push(AgentStage::LLMInference);
        let llm_result = self
            .llm
            .complete(crate::llm::provider::LLMRequest {
                system_prompt: prompt.system,
                user_message: prompt.user,
                max_tokens: 1024,
            })
            .await
            .map_err(|e| format!("LLM error: {e:?}"))?;

        // 4. Reasoning trace — system-side: mirrors the context evidence that was
        //    actually selected (same source as the evidence gate above).
        let evidence_refs: Vec<String> = ctx_result.context.evidence.iter().map(|e| e.source_id.clone()).collect();
        let reasoning = build_trace(evidence_refs.clone(), ctx_result.confidence);

        // 5. Context summary
        let context_summary = ContextSummary {
            decisions_count: ctx_result.context.decisions.len() as u32,
            reflections_count: ctx_result.context.reflections.len() as u32,
            memories_count: ctx_result.context.memories.len() as u32,
            patterns_count: ctx_result.context.patterns.len() as u32,
            evidence_refs: evidence_refs.clone(),
        };

        // 6. Assemble response (soft insufficient-evidence output)
        stages.push(AgentStage::Completed);
        let response = AgentResponse {
            answer: llm_result.text,
            reasoning,
            context: context_summary,
            context_id: ctx_result.snapshot_id,
            execution: ExecutionMetadata {
                mode: request.mode,
                model: self.llm.capability().model_name,
                prompt_version: prompt.version,
                reasoning_version: "v1".into(),
                generated_at: now,
                latency_ms: (now_ms() - start) as u64,
                stages,
            },
            session_id: request.session_id,
            insufficient_evidence,
            disclaimer,
        };
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ContextProvider;
    use crate::llm::provider::{LLMError, LLMProvider, LLMRequest, LLMResponse, ModelCapability};
    use crate::types::{AgentMode, ContextResult};
    use async_trait::async_trait;
    use context_engine::types::{AgentContext, CognitiveStage, ContextEvidence, DesiredOutcome, Intent};

    /// Fake context provider: returns an AgentContext with exactly `k` evidence items.
    struct FakeContext {
        k: usize,
    }

    fn intent() -> Intent {
        Intent {
            intent_type: "decision_support".into(),
            stage: CognitiveStage::Explore,
            desired_outcome: DesiredOutcome::Recommendation,
            domain: Some("investment".into()),
            action: None,
            entity: None,
        }
    }

    fn context_with_evidence(k: usize) -> AgentContext {
        let evidence: Vec<ContextEvidence> = (1..=k)
            .map(|i| ContextEvidence {
                source_type: "decision".into(),
                source_id: format!("DEC-{i:06}"),
                selection_reason: "matched".into(),
                relevance_score: 1.0,
            })
            .collect();
        AgentContext {
            snapshot_id: "CTX-1".into(),
            query: "test".into(),
            intent: intent(),
            evidence,
            decisions: vec![],
            reflections: vec![],
            memories: vec![],
            patterns: vec![],
            confidence: context_engine::types::ContextConfidence {
                overall: 0.5,
                coverage: 0.1,
                data_quality: 0.0,
                recency: 0.5,
                consistency: 0.8,
            },
            engine_version: "test".into(),
            generated_at: 0,
        }
    }

    #[async_trait(?Send)]
    impl ContextProvider for FakeContext {
        async fn build_context(&self, _query: &str) -> Result<ContextResult, String> {
            let context = context_with_evidence(self.k);
            let confidence = context.confidence.overall;
            Ok(ContextResult { snapshot_id: context.snapshot_id.clone(), context, confidence })
        }
    }

    /// Fake LLM: "claims" five evidence citations in its text regardless of how
    /// many were actually provided — the model-controlled channel that must NOT
    /// influence the insufficient-evidence flag.
    struct FakeLlm;

    #[async_trait(?Send)]
    impl LLMProvider for FakeLlm {
        fn capability(&self) -> ModelCapability {
            ModelCapability {
                provider: "noop".into(),
                model_name: "noop".into(),
                context_window: 0,
                supports_json: false,
            }
        }
        async fn complete(&self, _request: LLMRequest) -> Result<LLMResponse, LLMError> {
            Ok(LLMResponse {
                text: "Recommendation grounded on DEC-000001, DEC-000002, DEC-000003, DEC-000004, DEC-000005.".into(),
                finish_reason: "stop".into(),
                usage: None,
            })
        }
    }

    fn run(k: usize) -> Result<AgentResponse, String> {
        let runtime = AgentRuntime::new(Box::new(FakeContext { k }), Box::new(FakeLlm));
        futures::executor::block_on(runtime.execute(AgentRequest {
            query: "test".into(),
            mode: AgentMode::DecisionAdvisor,
            session_id: None,
            options: None,
        }))
    }

    #[test]
    fn evidence_below_threshold_is_ok_with_flag_and_disclaimer() {
        // context evidence = 4 (< 5) but the LLM's text claims 5 citations →
        // still insufficient: model output cannot bypass the system-side gate.
        let resp = run(4).expect("insufficient evidence must be Ok, not Err");
        assert!(resp.insufficient_evidence);
        assert_eq!(resp.disclaimer.as_deref(), Some(crate::policy::INSUFFICIENT_EVIDENCE_DISCLAIMER));
        assert!(resp.execution.stages.contains(&AgentStage::Completed));
    }

    #[test]
    fn empty_evidence_is_ok_not_err() {
        // 0 evidence is a valid Advisor output (HTTP 200), NOT an error — guards
        // against reintroducing an empty-evidence → Err hard gate in the runtime.
        let resp = run(0).expect("0 evidence must be Ok, not Err");
        assert!(resp.insufficient_evidence);
        assert!(resp.disclaimer.is_some());
    }

    #[test]
    fn evidence_at_threshold_is_sufficient() {
        let resp = run(5).expect("threshold evidence must be Ok");
        assert!(!resp.insufficient_evidence);
        assert!(resp.disclaimer.is_none());
        assert!(resp.execution.stages.contains(&AgentStage::Completed));
    }
}
