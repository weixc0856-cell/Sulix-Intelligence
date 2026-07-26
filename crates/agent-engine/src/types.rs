use serde::{Deserialize, Serialize};

pub type SessionId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    pub query: String,
    pub mode: AgentMode,
    pub session_id: Option<SessionId>,
    pub options: Option<AgentRequestOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequestOptions {
    pub include_evidence: Option<bool>,
    pub max_context_items: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentMode {
    DecisionAdvisor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub answer: String,
    pub reasoning: ReasoningTrace,
    pub context: ContextSummary,
    pub context_id: String,
    pub execution: ExecutionMetadata,
    pub session_id: Option<SessionId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSummary {
    pub decisions_count: u32,
    pub reflections_count: u32,
    pub memories_count: u32,
    pub patterns_count: u32,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetadata {
    pub mode: AgentMode,
    pub model: String,
    pub prompt_version: String,
    pub reasoning_version: String,
    pub generated_at: i64,
    pub latency_ms: u64,
    pub stages: Vec<AgentStage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentStage {
    ContextBuilding,
    PromptConstruction,
    LLMInference,
    ResponseValidation,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningTrace {
    pub confidence: f64,
    pub evidence_refs: Vec<String>,
    pub assumptions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub reasoning_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltPrompt {
    pub system: String,
    pub user: String,
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct ContextResult {
    pub context: context_engine::types::AgentContext,
    pub snapshot_id: String,
    pub confidence: f64,
}
