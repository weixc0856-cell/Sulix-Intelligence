# Agent Reasoning Engine — Sprint 5.7 Design Spec

## Context

Sprint 5.0-5.6 完成了完整的认知数据闭环（Signal → Decision → Outcome → Reflection → Memory → Context）。Sprint 5.7 构建 **Agent Reasoning Engine**——让 Sulix 首次基于自身认知历史生成具有个人连续性的推理回答。

### 定位

不是 Chat Bot。不是 ChatGPT UI。不是 Autonomous Agent。

**Agent Reasoning Engine** 是 Sulix 从 Intelligence Archive 进入 Personal Intelligence Agent 的关键跃迁：

```
Sprint 5.6: Cognitive Context Engine
    ↓ "系统能理解自己的历史"
Sprint 5.7: Agent Reasoning Engine
    ↓ "系统能基于历史进行推理"
Sprint 5.8: Chat Experience
    ↓ "用户能与系统对话"
```

### 核心原则

1. **不做 Chat UI** — 纯后端 API，前端 Sprint 5.8 独立开发
2. **不做 Conversation Memory** — `session_id` 轻量标识，不持久化
3. **不做 Tool Calling** — 只回答问题，不执行操作
4. **不做 Autonomous Loop** — `query → context → prompt → answer`，无多轮自治
5. **只实现 `decision_advisor` 模式** — 为未来 `strategy_review` / `reflection_coach` 等模式预留接口

---

## Section 1: Architecture

### 模块结构

```
crates/agent-engine/

├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── types.rs              ← AgentRequest, AgentResponse, ReasoningTrace
│   ├── runtime.rs            ← AgentRuntime (orchestrator)
│   ├── context.rs            ← ContextProvider trait (abstracts ContextEngine)
│   ├── prompt.rs             ← PromptBuilder + PromptTemplate (versioned)
│   ├── policy.rs             ← ReasoningPolicy per mode
│   ├── reasoning.rs          ← ReasoningTrace builder
│   ├── validator.rs          ← ResponseValidator trait
│   └── llm/
│       ├── provider.rs       ← LLMProvider trait + ModelCapability
│       ├── deepseek.rs       ← DeepSeek provider
│       ├── openrouter.rs     ← OpenRouter provider
│       └── noop.rs           ← Noop provider (testing)

api/src/
    routes/
        agent.rs              ← POST /api/internal/agent/run
```

### 数据流

```
POST /api/internal/agent/run { query, mode: "decision_advisor" }
    ↓
1. AgentRuntime.execute()
    │
    ├── 2. ContextEngine.build(query)
    │       → AgentContext (decisions, reflections, memories, patterns)
    │
    ├── 3. PromptBuilder.build(context, mode, query)
    │       → system prompt + context injection + query
    │
    ├── 4. LLMProvider.complete(prompt)
    │       → LLM response text
    │
    └── 5. ResponseParser.parse(answer, context)
            → AgentResponse { answer, confidence, evidence_refs, context_id, mode }
    ↓
AgentResponse JSON
```

---

## Section 2: Data Model

### Core Types

```rust
/// Request correlation only. Not persisted.
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
    // Future: StrategyReview, ReflectionCoach, ResearchAssistant
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

/// Structured reasoning trace — allows users to understand WHY the answer was given.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningTrace {
    pub confidence: f64,
    pub evidence_refs: Vec<String>,
    pub assumptions: Vec<String>,
    pub uncertainty: Vec<String>,
}

/// AgentRuntime depends on ContextProvider (abstract), not ContextBuilder<D1Store>.
#[async_trait(?Send)]
pub trait ContextProvider {
    async fn build_context(&self, query: &str) -> Result<AgentContext, String>;
}
```

### LLM Provider Trait

```rust
#[async_trait(?Send)]
pub trait LLMProvider {
    fn capability(&self) -> ModelCapability;
    async fn complete(&self, request: LLMRequest) -> Result<LLMResponse, String>;
}

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

/// Prompt template with version tracking.
pub struct PromptTemplate {
    pub version: String,           // "decision_advisor.v1"
    pub system_prompt: String,
}

/// ReasoningPolicy — configures agent behavior per mode.
pub struct ReasoningPolicy {
    pub context_budget: u32,
    pub evidence_requirement: EvidencePolicy,
    pub confidence_threshold: f64,
}

pub enum EvidencePolicy {
    Required,
    Preferred,
    Optional,
}
```

---

## Section 3: PromptBuilder

### Prompt 结构

```
SYSTEM PROMPT (decision_advisor.v1):
You are Sulix Intelligence Agent — a personal decision intelligence assistant.
You have access to the user's personal decision history, reflections, and learned patterns.
Rules:
- Always cite specific past decisions/reflections/memories as evidence
- Distinguish between facts and assumptions
- Mention uncertainty when evidence is insufficient
- Connect patterns across multiple decisions
- Be concise but specific

CONTEXT:
{serialized AgentContext}

USER QUERY:
{query}
```

### PromptBuilder

```rust
pub struct PromptBuilder;

impl PromptBuilder {
    pub fn build(&self, context: &AgentContext, mode: &AgentMode, query: &str) -> (String, String) {
        // Returns (system_prompt, user_message)
    }
}
```

---

## Section 4: Agent Policy

```rust
pub struct AgentPolicy {
    pub mode: AgentMode,
    pub max_context_items: u32,
    pub require_evidence: bool,
    pub confidence_threshold: f64,
}

impl AgentPolicy {
    pub fn for_mode(mode: &AgentMode) -> Self {
        match mode {
            AgentMode::DecisionAdvisor => Self {
                mode: mode.clone(),
                max_context_items: 15,
                require_evidence: true,
                confidence_threshold: 0.5,
            },
        }
    }
}
```

---

## Section 5: API

```
POST /api/internal/agent/run
Content-Type: application/json

Request:
{
  "query": "Should I invest in AI startups?",
  "mode": "decision_advisor",
  "session_id": null
}

Response:
{
  "answer": "基于你的决策历史，你有 3 次与 AI 相关的投资决策...",
  "reasoning": {
    "confidence": 0.82,
    "evidence_refs": ["DEC-001", "MEM-003", "REF-002"],
    "assumptions": ["假设当前市场趋势持续"],
    "uncertainty": ["缺少长期市场数据"]
  },
  "context_id": "CTX-1710000000",
  "mode": "decision_advisor",
  "model": "deepseek-chat",
  "prompt_version": "decision_advisor.v1",
  "session_id": null,
  "generated_at": 1710000000
}
```

---

## Section 6: Sprint 边界

### 做

- `crates/agent-engine/` — AgentRuntime, ContextProvider trait, LLMProvider trait, PromptBuilder (+ PromptTemplate version), ReasoningPolicy, ReasoningTrace, ResponseValidator
- `LLMProvider` trait + ModelCapability + Noop/DeepSeek/OpenRouter providers
- `ContextProvider` trait（抽象 ContextEngine，不直接依赖 D1Store）
- `ReasoningTrace` — confidence, evidence_refs, assumptions, uncertainty
- `ResponseValidator` — 检查 evidence 要求、confidence 阈值
- `POST /api/internal/agent/run` 端点
- `decision_advisor` 模式（`decision_advisor.v1` prompt）
- Context Engine 集成（通过 ContextProvider trait）
- Evidence 追溯（context_id + evidence_refs + prompt_version）
- `session_id` 轻量标识（不持久化）

### 不做

- Chat UI（Astro 前端，Sprint 5.8）
- Conversation Memory / 消息持久化
- Tool Calling / Agent Action
- Autonomous Loop / Multi-step Reasoning
- Multi-modal / Streaming
- 除 `decision_advisor` 外的 Agent Mode

---

## Section 7: Verification

1. `cargo check --workspace` + `cargo test --workspace`
2. **Context injection test**: AgentRequest → PromptBuilder 生成的 prompt 包含 decisions/reflections/memories
3. **Provider swap test**: Noop → DeepSeek → OpenRouter 接口不变，capability 正确
4. **Evidence trace test**: Response 包含 context_id + evidence_refs + prompt_version
5. **ReasoningTrace test**: response.reasoning 包含 confidence + evidence_refs + assumptions + uncertainty
6. **Mode dispatch test**: decision_advisor → ReasoningPolicy 正确配置
7. **ResponseValidator test**: no evidence → fails Required; has evidence → passes
8. **ContextProvider abstraction test**: mock provider → AgentRuntime 正常工作
9. **API integration test**: curl 发送请求 → 接收结构化 AgentResponse
