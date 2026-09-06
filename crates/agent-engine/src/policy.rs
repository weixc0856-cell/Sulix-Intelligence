use crate::types::AgentMode;

/// Reasoning policy for an Agent run.
///
/// `min_evidence_items` is the Advisor product-layer **insufficient-evidence
/// threshold** — nothing else. Fewer than this many matched decisions ⇒ the
/// response is flagged `insufficient_evidence` and carries a disclaimer. It is
/// NOT a confidence threshold and MUST NOT feed the confidence calculation
/// (the confidence formula's low resolution is a known limitation, tracked
/// separately from the Advisor evidence gate).
pub struct ReasoningPolicy {
    /// Fewer than this many context evidence items ⇒ insufficient evidence.
    pub min_evidence_items: u32,
    /// Prompt template version tag, carried into the built prompt and response.
    pub prompt_version: &'static str,
}

/// Disclaimer attached when the Advisor has fewer than `min_evidence_items`
/// matched decisions to ground its recommendation on.
pub const INSUFFICIENT_EVIDENCE_DISCLAIMER: &str = "This recommendation is based on limited decision history.";

impl ReasoningPolicy {
    pub fn for_mode(mode: &AgentMode) -> Self {
        match mode {
            AgentMode::DecisionAdvisor => Self { min_evidence_items: 5, prompt_version: "decision_advisor.v1" },
        }
    }

    /// True when `evidence_count` falls below the Advisor evidence threshold.
    pub fn insufficient(&self, evidence_count: u32) -> bool {
        evidence_count < self.min_evidence_items
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AgentMode;

    #[test]
    fn insufficient_boundary() {
        let policy = ReasoningPolicy::for_mode(&AgentMode::DecisionAdvisor);
        assert_eq!(policy.min_evidence_items, 5);
        assert!(policy.insufficient(0));
        assert!(policy.insufficient(4));
        assert!(!policy.insufficient(5));
        assert!(!policy.insufficient(9));
    }

    #[test]
    fn disclaimer_is_present() {
        assert!(!INSUFFICIENT_EVIDENCE_DISCLAIMER.is_empty());
    }
}
