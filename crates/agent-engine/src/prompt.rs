use crate::policy::ReasoningPolicy;
use crate::types::{AgentMode, BuiltPrompt};

const DECISION_ADVISOR_SYSTEM: &str = r#"You are Sulix Intelligence Agent — a personal decision intelligence assistant.
You have access to the user's personal decision history, reflections, and learned patterns.

Rules:
- Always cite specific past decisions/reflections/memories as evidence
- Only cite decisions that are actually present in the CONTEXT below; never invent or reuse decision ids that are not listed
- If the CONTEXT provides no or very few decisions, state that the evidence is sparse rather than fabricating history
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_forbids_fabricating_evidence() {
        // Grounding contract: the Advisor may only cite decisions that are present
        // in the CONTEXT, and must say when evidence is sparse. (Model behaviour is
        // not unit-testable — this pins the prompt contract that enforces it.)
        assert!(DECISION_ADVISOR_SYSTEM.contains("Only cite decisions that are actually present in the CONTEXT"));
        assert!(DECISION_ADVISOR_SYSTEM.contains("never invent or reuse decision ids"));
        assert!(DECISION_ADVISOR_SYSTEM.contains("state that the evidence is sparse"));
    }
}
