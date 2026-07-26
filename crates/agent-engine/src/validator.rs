use crate::policy::EvidencePolicy;
use crate::types::AgentResponse;
use async_trait::async_trait;

pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

#[async_trait(?Send)]
pub trait ResponseValidator {
    async fn validate(&self, response: &AgentResponse, evidence_policy: &EvidencePolicy) -> ValidationResult;
}

pub struct DefaultValidator;

#[async_trait(?Send)]
impl ResponseValidator for DefaultValidator {
    async fn validate(&self, response: &AgentResponse, evidence_policy: &EvidencePolicy) -> ValidationResult {
        let mut errors = Vec::new();
        if let EvidencePolicy::Required = evidence_policy {
            if response.reasoning.evidence_refs.is_empty() {
                errors.push("evidence required but evidence_refs is empty".into());
            }
        }
        if response.reasoning.confidence < 0.4 {
            errors.push("confidence below threshold — add insufficient evidence disclaimer".into());
        }
        ValidationResult { valid: errors.is_empty(), errors }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AgentMode, AgentResponse, ContextSummary, ExecutionMetadata, ReasoningTrace};

    fn make_response(evidence: Vec<String>, confidence: f64) -> AgentResponse {
        AgentResponse {
            answer: "test".into(),
            context: ContextSummary {
                decisions_count: 0,
                reflections_count: 0,
                memories_count: 0,
                patterns_count: 0,
                evidence_refs: vec![],
            },
            context_id: "CTX-1".into(),
            reasoning: ReasoningTrace {
                confidence,
                evidence_refs: evidence,
                assumptions: vec![],
                uncertainty: vec![],
                reasoning_version: "v1".into(),
            },
            execution: ExecutionMetadata {
                mode: AgentMode::DecisionAdvisor,
                model: "noop".into(),
                prompt_version: "test".into(),
                reasoning_version: "v1".into(),
                generated_at: 0,
                latency_ms: 0,
                stages: vec![],
            },
            session_id: None,
        }
    }

    #[test]
    fn required_evidence_passes() {
        let r = make_response(vec!["DEC-001".into()], 0.8);
        let v = futures::executor::block_on(DefaultValidator.validate(&r, &EvidencePolicy::Required));
        assert!(v.valid);
    }

    #[test]
    fn required_evidence_fails_empty() {
        let r = make_response(vec![], 0.8);
        let v = futures::executor::block_on(DefaultValidator.validate(&r, &EvidencePolicy::Required));
        assert!(!v.valid);
    }

    #[test]
    fn low_confidence_fails() {
        let r = make_response(vec!["DEC-001".into()], 0.3);
        let v = futures::executor::block_on(DefaultValidator.validate(&r, &EvidencePolicy::Preferred));
        assert!(!v.valid);
    }
}
