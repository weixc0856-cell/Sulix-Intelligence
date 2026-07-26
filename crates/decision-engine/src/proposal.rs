//! Decision Proposal builder — generates structured proposals from signals + claims.

use crate::domain::{DecisionProposal, ProposalClaimRef};

/// Build a DecisionProposal from signal context and claims.
/// In v1, this is a simple builder; future versions will use ModelProvider.
pub fn build_proposal(signal_id: i64, signal_title: &str, claims: Vec<ProposalClaimRef>) -> DecisionProposal {
    let evidence_count = claims.len();
    let confidence = (evidence_count as f64 * 0.15).clamp(0.1, 0.8);

    DecisionProposal {
        signal_id,
        signal_title: signal_title.to_string(),
        recommended_action: format!("Review {} for potential action", signal_title),
        alternatives: vec!["Monitor and wait".into(), "Gather additional evidence".into()],
        rationale: format!("Based on {} supporting claims from the signal \"{}\".", evidence_count, signal_title),
        confidence,
        risks: vec!["Insufficient evidence for high-confidence action".into()],
        supporting_claims: claims,
    }
}
