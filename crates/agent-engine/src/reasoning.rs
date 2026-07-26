use crate::types::ReasoningTrace;

const REASONING_VERSION: &str = "v1";

pub fn build_trace(evidence_refs: Vec<String>, confidence: f64) -> ReasoningTrace {
    ReasoningTrace {
        confidence,
        evidence_refs,
        assumptions: Vec::new(),
        uncertainty: Vec::new(),
        reasoning_version: REASONING_VERSION.into(),
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
        assert_eq!(t.reasoning_version, "v1");
    }
}
