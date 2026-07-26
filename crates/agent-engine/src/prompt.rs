use crate::policy::ReasoningPolicy;
use crate::types::{AgentMode, BuiltPrompt};

const DECISION_ADVISOR_SYSTEM: &str = r#"You are Sulix Intelligence Agent — a personal decision intelligence assistant.
You have access to the user's personal decision history, reflections, and learned patterns.

Rules:
- Always cite specific past decisions/reflections/memories as evidence
- Distinguish between facts and assumptions
- Mention uncertainty when evidence is insufficient
- Connect patterns across multiple decisions
- Be concise but specific"#;

pub struct PromptBuilder;

impl PromptBuilder {
    pub fn build(&self, _context: &context_engine::types::AgentContext, mode: &AgentMode, query: &str) -> BuiltPrompt {
        let policy = ReasoningPolicy::for_mode(mode);
        let system = format!("{}\n\nPrompt version: {}", DECISION_ADVISOR_SYSTEM, policy.prompt_version);
        let context_json = serde_json::to_string_pretty(_context).unwrap_or_default();
        let user = format!("CONTEXT:\n{}\n\nUSER QUERY:\n{}", context_json, query);
        BuiltPrompt { system, user, version: policy.prompt_version.into() }
    }
}
