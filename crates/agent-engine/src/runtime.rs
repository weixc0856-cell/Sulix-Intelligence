use crate::context::ContextProvider;
use crate::llm::provider::LLMProvider;
use crate::policy::ReasoningPolicy;
use crate::prompt::PromptBuilder;
use crate::reasoning::build_trace;
use crate::types::{AgentRequest, AgentResponse, AgentStage, ContextSummary, ExecutionMetadata};
use crate::validator::{DefaultValidator, ResponseValidator};

pub struct AgentRuntime {
    context: Box<dyn ContextProvider>,
    llm: Box<dyn LLMProvider>,
    prompt_builder: PromptBuilder,
    validator: Box<dyn ResponseValidator>,
}

impl AgentRuntime {
    pub fn new(context: Box<dyn ContextProvider>, llm: Box<dyn LLMProvider>) -> Self {
        Self { context, llm, prompt_builder: PromptBuilder, validator: Box::new(DefaultValidator) }
    }

    pub async fn execute(&self, request: AgentRequest) -> Result<AgentResponse, String> {
        let start = js_sys::Date::now();
        let now = (start / 1000.0) as i64;
        let policy = ReasoningPolicy::for_mode(&request.mode);
        let mut stages = Vec::new();

        // 1. Context
        stages.push(AgentStage::ContextBuilding);
        let ctx_result = self.context.build_context(&request.query).await?;

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

        // 4. Reasoning trace
        let evidence_refs: Vec<String> = ctx_result.context.evidence.iter().map(|e| e.source_id.clone()).collect();
        let reasoning = build_trace(evidence_refs.clone(), ctx_result.confidence);

        // 5. Build context summary
        let context_summary = ContextSummary {
            decisions_count: ctx_result.context.decisions.len() as u32,
            reflections_count: ctx_result.context.reflections.len() as u32,
            memories_count: ctx_result.context.memories.len() as u32,
            patterns_count: ctx_result.context.patterns.len() as u32,
            evidence_refs: evidence_refs.clone(),
        };

        // 6. Assemble response (before validation so we can validate it)
        stages.push(AgentStage::ResponseValidation);
        let mut response = AgentResponse {
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
                latency_ms: (js_sys::Date::now() - start) as u64,
                stages,
            },
            session_id: request.session_id,
        };

        // 6. Validate
        let validation = self.validator.validate(&response, &policy.evidence_policy).await;
        if !validation.valid {
            return Err(format!("response validation: {}", validation.errors.join("; ")));
        }

        // 7. Mark completed (stages is owned by response.execution.stages)
        response.execution.stages.push(AgentStage::Completed);
        Ok(response)
    }
}
