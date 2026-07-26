use crate::types::AgentMode;

pub enum EvidencePolicy {
    Required,
    Preferred,
    Optional,
}

pub struct ReasoningPolicy {
    pub evidence_policy: EvidencePolicy,
    pub max_context_items: u32,
    pub confidence_threshold: f64,
    pub require_uncertainty: bool,
    pub prompt_version: &'static str,
}

impl ReasoningPolicy {
    pub fn for_mode(mode: &AgentMode) -> Self {
        match mode {
            AgentMode::DecisionAdvisor => Self {
                evidence_policy: EvidencePolicy::Required,
                max_context_items: 15,
                confidence_threshold: 0.5,
                require_uncertainty: true,
                prompt_version: "decision_advisor.v1",
            },
        }
    }
}
