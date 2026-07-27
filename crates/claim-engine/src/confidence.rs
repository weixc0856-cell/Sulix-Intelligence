//! ClaimConfidenceEvaluator — computes confidence scores for claims.
//!
//! Uses the ConfidenceEngine v2 factors (not LLM's stated uncertainty).
//! Confidence is a function of evidence quality, source trust, and freshness.

use store::domain::confidence::calculator::calculate;
use store::domain::confidence::factors::{ConfidenceFactors, ConfidenceResult};
use store::domain::confidence::policy::ConfidencePolicy;

use crate::domain::ClaimCandidate;

/// Compute the confidence score for a claim candidate.
/// Does NOT use LLM uncertainty — uses evidence factors.
pub fn evaluate_claim_confidence(candidate: &ClaimCandidate, source_trust: f64, freshness: f64) -> ConfidenceResult {
    let evidence_count = candidate.evidence_refs.len() as u32;
    let avg_strength = if evidence_count > 0 {
        candidate.evidence_refs.iter().map(|r| r.relevance).sum::<f64>() / evidence_count as f64
    } else {
        0.0
    };

    let factors = ConfidenceFactors {
        evidence_count,
        evidence_strength: avg_strength,
        source_trust,
        freshness,
        calibration_score: 0.5, // starts neutral; updated as outcomes come in
    };

    calculate(&factors, &ConfidencePolicy::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ClaimType, EvidenceRef, Uncertainty};

    #[test]
    fn high_evidence_high_confidence() {
        let candidate = ClaimCandidate {
            statement: "Test claim".into(),
            claim_type: ClaimType::Fact,
            reasoning: "test".into(),
            falsification: "".into(),
            evidence_refs: vec![EvidenceRef { article_id: 1, relevance: 0.9 }; 5],
            counter_arguments: vec![],
            frameworks_applied: vec![],
            uncertainty: Uncertainty::Low,
        };
        let result = evaluate_claim_confidence(&candidate, 0.85, 1.0);
        assert!(result.score > 0.5, "score should be high: {:.3}", result.score);
    }

    #[test]
    fn no_evidence_low_confidence() {
        let candidate = ClaimCandidate {
            statement: "Weak claim".into(),
            claim_type: ClaimType::Opinion,
            reasoning: "".into(),
            falsification: "".into(),
            evidence_refs: vec![],
            counter_arguments: vec![],
            frameworks_applied: vec![],
            uncertainty: Uncertainty::High,
        };
        let result = evaluate_claim_confidence(&candidate, 0.3, 0.2);
        assert!(result.score < 0.3, "score should be low: {:.3}", result.score);
    }
}
