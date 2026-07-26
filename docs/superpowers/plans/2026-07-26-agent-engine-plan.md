# Agent Reasoning Engine (Sprint 5.7) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development to implement this plan task-by-task.

**Goal:** Build the Agent Reasoning Engine — Sulix's first capability to generate reasoned answers grounded in personal Decision/Reflection/Memory history.

**Architecture:** New `crates/agent-engine/` with ContextProvider → PromptBuilder → LLMProvider → ReasoningTrace → ResponseValidator pipeline. ContextProvider abstracts the ContextEngine (no D1Store dependency). POST /api/internal/agent/run endpoint. DecisionAdvisor mode only.

**Tech Stack:** Rust + Cloudflare Workers + existing ContextEngine + HttpSummarizer (LLM)

**Spec reference:** `docs/superpowers/specs/2026-07-26-agent-engine-design.md`

---

## File Structure

### New files:
- `crates/agent-engine/Cargo.toml`
- `crates/agent-engine/src/lib.rs`
- `crates/agent-engine/src/types.rs` — AgentRequest, AgentMode, AgentResponse, ReasoningTrace
- `crates/agent-engine/src/runtime.rs` — AgentRuntime
- `crates/agent-engine/src/context.rs` — ContextProvider trait
- `crates/agent-engine/src/prompt.rs` — PromptBuilder + PromptTemplate (versioned)
- `crates/agent-engine/src/policy.rs` — ReasoningPolicy + EvidencePolicy
- `crates/agent-engine/src/reasoning.rs` — ReasoningTrace builder
- `crates/agent-engine/src/validator.rs` — ResponseValidator trait
- `crates/agent-engine/src/llm/provider.rs` — LLMProvider trait + ModelCapability + LLMRequest/LLMResponse
- `crates/agent-engine/src/llm/noop.rs` — NoopLLM for testing
- `crates/agent-engine/src/llm/deepseek.rs` — DeepSeek provider (future)
- `crates/agent-engine/src/llm/openrouter.rs` — OpenRouter provider (future)
- `crates/api/src/routes/agent.rs` — POST /api/internal/agent/run

### Existing files to modify:
- `Cargo.toml` (workspace) — add agent-engine member + dep
- `crates/api/Cargo.toml` — add agent-engine dep
- `crates/api/src/lib.rs` — register agent route
- `crates/api/src/routes/mod.rs` — register agent module

---

## Task Plan

### Task 1: agent-engine crate skeleton + types

**Files:**
- Create: `crates/agent-engine/Cargo.toml`
- Create: `crates/agent-engine/src/lib.rs`
- Create: `crates/agent-engine/src/types.rs`
- Modify: `Cargo.toml` (workspace)

Cargo.toml depends on: worker, store, context-engine, serde, serde_json, async-trait, thiserror.

`lib.rs`:
```rust
pub mod context;
pub mod llm;
pub mod policy;
pub mod prompt;
pub mod reasoning;
pub mod runtime;
pub mod types;
pub mod validator;
```

`types.rs` — all types from spec:
```rust
use serde::{Serialize, Deserialize};

pub type SessionId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    pub query: String,
    pub mode: AgentMode,
    pub session_id: Option<SessionId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentMode {
    DecisionAdvisor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub answer: String,
    pub reasoning: ReasoningTrace,
    pub context_id: String,
    pub mode: AgentMode,
    pub model: String,
    pub prompt_version: String,
    pub session_id: Option<SessionId>,
    pub generated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningTrace {
    pub confidence: f64,
    pub evidence_refs: Vec<String>,
    pub assumptions: Vec<String>,
    pub uncertainty: Vec<String>,
}
```

Register workspace member + dep. Commit.

### Task 2: LLMProvider trait + NoopLLM

**Files:**
- Create: `crates/agent-engine/src/llm/provider.rs`
- Create: `crates/agent-engine/src/llm/noop.rs`

`provider.rs`:
```rust
use async_trait::async_trait;
use serde::{Serialize, Deserialize};

pub struct ModelCapability {
    pub model_name: String,
    pub context_window: u32,
    pub supports_json: bool,
    pub supports_streaming: bool,
}

pub struct LLMRequest {
    pub system_prompt: String,
    pub user_message: String,
    pub max_tokens: u32,
}

pub struct LLMResponse {
    pub text: String,
    pub finish_reason: String,
    pub usage: Option<LLMUsage>,
}

pub struct LLMUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

#[async_trait(?Send)]
pub trait LLMProvider {
    fn capability(&self) -> ModelCapability;
    async fn complete(&self, request: LLMRequest) -> Result<LLMResponse, String>;
}
```

`noop.rs`:
```rust
use crate::llm::provider::{LLMProvider, LLMRequest, LLMResponse, ModelCapability};

pub struct NoopLLM;

impl LLMProvider for NoopLLM {
    fn capability(&self) -> ModelCapability {
        ModelCapability { model_name: "noop".into(), context_window: 4096, supports_json: false, supports_streaming: false }
    }
    async fn complete(&self, _request: LLMRequest) -> Result<LLMResponse, String> {
        Ok(LLMResponse { text: "Noop response — LLM not configured.".into(), finish_reason: "stop".into(), usage: None })
    }
}
```

Add `pub mod provider; pub mod noop;` to `lib.rs`. Commit.

### Task 3: ContextProvider trait + Policy + PromptBuilder

**Files:**
- Create: `crates/agent-engine/src/context.rs`
- Create: `crates/agent-engine/src/policy.rs`
- Create: `crates/agent-engine/src/prompt.rs`

`context.rs`:
```rust
use async_trait::async_trait;
use context_engine::types::AgentContext;

#[async_trait(?Send)]
pub trait ContextProvider {
    async fn build_context(&self, query: &str) -> Result<AgentContext, String>;
}
```

`policy.rs`:
```rust
use crate::types::AgentMode;

pub enum EvidencePolicy { Required, Preferred, Optional }

pub struct ReasoningPolicy {
    pub context_budget: u32,
    pub evidence_requirement: EvidencePolicy,
    pub confidence_threshold: f64,
    pub prompt_version: &'static str,
}

impl ReasoningPolicy {
    pub fn for_mode(mode: &AgentMode) -> Self {
        match mode {
            AgentMode::DecisionAdvisor => Self {
                context_budget: 15,
                evidence_requirement: EvidencePolicy::Required,
                confidence_threshold: 0.5,
                prompt_version: "decision_advisor.v1",
            },
        }
    }
}
```

`prompt.rs`:
```rust
use context_engine::types::AgentContext;
use crate::types::AgentMode;
use crate::policy::ReasoningPolicy;

pub struct PromptTemplate {
    pub version: String,
    pub system_prompt: String,
}

fn decision_advisor_system() -> String {
    r#"You are Sulix Intelligence Agent — a personal decision intelligence assistant.
You have access to the user's personal decision history, reflections, and learned patterns.
Your goal is to provide grounded, evidence-based responses.

Rules:
- Always cite specific past decisions/reflections/memories as evidence
- Distinguish between facts and assumptions
- Mention uncertainty when evidence is insufficient
- Connect patterns across multiple decisions
- Be concise but specific"#.into()
}

pub struct PromptBuilder;

impl PromptBuilder {
    pub fn build(&self, context: &AgentContext, mode: &AgentMode, query: &str) -> (String, String) {
        let policy = ReasoningPolicy::for_mode(mode);
        let system = match mode {
            AgentMode::DecisionAdvisor => format!("{}\n\nPrompt version: {}", decision_advisor_system(), policy.prompt_version),
        };
        let context_json = serde_json::to_string_pretty(context).unwrap_or_default();
        let user_msg = format!("CONTEXT:\n{}\n\nUSER QUERY:\n{}", context_json, query);
        (system, user_msg)
    }
}
```

Commit.

### Task 4: ReasoningTrace builder + ResponseValidator

**Files:**
- Create: `crates/agent-engine/src/reasoning.rs`
- Create: `crates/agent-engine/src/validator.rs`

`reasoning.rs`:
```rust
use crate::types::ReasoningTrace;

pub fn build_trace(evidence_refs: Vec<String>, confidence: f64) -> ReasoningTrace {
    ReasoningTrace {
        confidence,
        evidence_refs,
        assumptions: Vec::new(),    // parsed from LLM output in production
        uncertainty: Vec::new(),    // parsed from LLM output in production
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_contains_all_fields() {
        let t = build_trace(vec!["DEC-001".into()], 0.82);
        assert!((t.confidence - 0.82).abs() < 0.01);
        assert_eq!(t.evidence_refs.len(), 1);
    }
}
```

`validator.rs`:
```rust
use async_trait::async_trait;
use crate::types::AgentResponse;
use crate::policy::EvidencePolicy;

pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

#[async_trait(?Send)]
pub trait ResponseValidator {
    async fn validate(&self, response: &AgentResponse, evidence_policy: &EvidencePolicy) -> ValidationResult;
}

pub struct DefaultValidator;

impl ResponseValidator for DefaultValidator {
    async fn validate(&self, response: &AgentResponse, evidence_policy: &EvidencePolicy) -> ValidationResult {
        let mut errors = Vec::new();
        match evidence_policy {
            EvidencePolicy::Required => {
                if response.reasoning.evidence_refs.is_empty() {
                    errors.push("evidence_required but evidence_refs is empty".into());
                }
            }
            _ => {}
        }
        ValidationResult { valid: errors.is_empty(), errors }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ReasoningTrace, AgentResponse, AgentMode};
    use crate::policy::EvidencePolicy;

    fn make_response(evidence: Vec<String>) -> AgentResponse {
        AgentResponse {
            answer: "test".into(), context_id: "CTX-1".into(),
            reasoning: ReasoningTrace { confidence: 0.5, evidence_refs: evidence, assumptions: vec![], uncertainty: vec![] },
            mode: AgentMode::DecisionAdvisor, model: "noop".into(), prompt_version: "test".into(),
            session_id: None, generated_at: 0,
        }
    }

    #[test]
    fn required_evidence_passes() {
        let r = make_response(vec!["DEC-001".into()]);
        let v = DefaultValidator.validate(&r, &EvidencePolicy::Required).await;
        assert!(v.valid);
    }

    #[test]
    fn required_evidence_fails_empty() {
        let r = make_response(vec![]);
        let v = DefaultValidator.validate(&r, &EvidencePolicy::Required).await;
        assert!(!v.valid);
    }
}
```

Note: tests need `futures` or `block_on` for async. Run tests with `cargo test -p agent-engine`. Fix if needed. Commit.

### Task 5: AgentRuntime

**Files:**
- Create: `crates/agent-engine/src/runtime.rs`

```rust
use crate::context::ContextProvider;
use crate::llm::provider::LLMProvider;
use crate::prompt::PromptBuilder;
use crate::reasoning::build_trace;
use crate::types::{AgentMode, AgentRequest, AgentResponse};
use crate::validator::{DefaultValidator, ResponseValidator};
use crate::policy::ReasoningPolicy;

pub struct AgentRuntime {
    context: Box<dyn ContextProvider>,
    llm: Box<dyn LLMProvider>,
    prompt_builder: PromptBuilder,
    validator: Box<dyn ResponseValidator>,
}

impl AgentRuntime {
    pub fn new(
        context: Box<dyn ContextProvider>,
        llm: Box<dyn LLMProvider>,
    ) -> Self {
        Self {
            context,
            llm,
            prompt_builder: PromptBuilder,
            validator: Box::new(DefaultValidator),
        }
    }

    pub async fn execute(&self, request: AgentRequest) -> Result<AgentResponse, String> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let policy = ReasoningPolicy::for_mode(&request.mode);

        // 1. Build context
        let agent_context = self.context.build_context(&request.query).await?;

        // 2. Build prompt
        let (system, user) = self.prompt_builder.build(&agent_context, &request.mode, &request.query);

        // 3. Call LLM
        let llm_response = self.llm.complete(crate::llm::provider::LLMRequest {
            system_prompt: system,
            user_message: user,
            max_tokens: 1024,
        }).await?;

        // 4. Build reasoning trace
        let evidence_refs: Vec<String> = agent_context.evidence.iter().map(|e| e.source_id.clone()).collect();
        let reasoning = build_trace(evidence_refs, agent_context.confidence.overall);

        // 5. Assemble response
        let response = AgentResponse {
            answer: llm_response.text,
            reasoning,
            context_id: agent_context.snapshot_id.clone(),
            mode: request.mode,
            model: self.llm.capability().model_name,
            prompt_version: policy.prompt_version.into(),
            session_id: request.session_id,
            generated_at: now,
        };

        // 6. Validate
        let validation = self.validator.validate(&response, &policy.evidence_requirement).await;
        if !validation.valid {
            return Err(format!("response validation failed: {}", validation.errors.join("; ")));
        }

        Ok(response)
    }
}
```

Commit.

### Task 6: API route + wiring

**Files:**
- Create: `crates/api/src/routes/agent.rs`
- Modify: `crates/api/Cargo.toml`
- Modify: `crates/api/src/routes/mod.rs`
- Modify: `crates/api/src/lib.rs`

`agent.rs`:
```rust
use agent_engine::runtime::AgentRuntime;
use agent_engine::context::ContextProvider;
use agent_engine::llm::noop::NoopLLM;
use agent_engine::types::{AgentMode, AgentRequest, AgentResponse};
use context_engine::types::AgentContext;
use context_engine::builder::ContextBuilder;
use store::D1Store;
use worker::*;
use crate::shared::response;

pub async fn run(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let body: AgentRequest = match req.json().await {
        Ok(b) => b,
        Err(_) => return response::json_err(400, "invalid request body"),
    };

    let store = D1Store::new(ctx.env.d1("DB")?);

    // Wrap ContextBuilder as ContextProvider
    struct CtxWrapper(ContextBuilder<D1Store>);
    #[async_trait::async_trait(?Send)]
    impl ContextProvider for CtxWrapper {
        async fn build_context(&self, query: &str) -> Result<AgentContext, String> {
            self.0.build(query, None, None).await
        }
    }

    let runtime = AgentRuntime::new(
        Box::new(CtxWrapper(ContextBuilder::new(store))),
        Box::new(NoopLLM),
    );

    match runtime.execute(body).await {
        Ok(response) => response::json_ok(serde_json::to_value(response).unwrap_or_default()),
        Err(e) => response::json_err(500, &e),
    }
}
```

Add route: `.post_async("/api/internal/agent/run", routes::agent::run)`

Add Cargo dep. Register module. Commit.

### Task 7: Full compilation + test

Run `cargo check --workspace` and `cargo test --workspace`. Fix issues. Push.
