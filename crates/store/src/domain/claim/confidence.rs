//! Confidence Calculator — 从 evidence strength 推导 claim confidence。
//!
//! 纯函数，无外部依赖。第一版算法：
//!
//! confidence = normalize(Σ supporting strength - Σ contradicting strength)
//!
//! 未来扩展方向：
//! - source reliability weighting
//! - time decay
//! - multi-agent agreement

use crate::ClaimEvidence;

/// 从一组 evidence 中推导 claim 的 confidence 分数。
///
/// 算法：
/// 1. 计算支持总分 = Σ supports_strength
/// 2. 计算反对总分 = Σ contradicts_strength + Σ weakens_strength * 0.5
/// 3. raw = 支持总分 - 反对总分
/// 4. 归一化到 0~1：sigmoid(raw) = 1 / (1 + e^(-raw))
pub fn calculate_confidence(evidence: &[ClaimEvidence]) -> f64 {
    if evidence.is_empty() {
        return 0.0;
    }

    let mut support: f64 = 0.0;
    let mut oppose: f64 = 0.0;

    for e in evidence {
        match e.relation {
            crate::EvidenceRelation::Supports => support += e.strength,
            crate::EvidenceRelation::Contradicts => oppose += e.strength,
            crate::EvidenceRelation::Weakens => oppose += e.strength * 0.5,
        }
    }

    let raw = support - oppose;
    // sigmoid 归一化到 0~1
    1.0 / (1.0 + (-raw).exp())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClaimEvidence, EvidenceRelation};

    #[test]
    fn empty_evidence_returns_zero() {
        assert_eq!(calculate_confidence(&[]), 0.0);
    }

    #[test]
    fn strong_support_high_confidence() {
        let ev =
            vec![ClaimEvidence { claim_id: 1, evidence_id: 1, strength: 3.0, relation: EvidenceRelation::Supports }];
        let c = calculate_confidence(&ev);
        assert!(c > 0.5, "expected >0.5, got {c}");
    }

    #[test]
    fn strong_contradiction_low_confidence() {
        let ev =
            vec![ClaimEvidence { claim_id: 1, evidence_id: 1, strength: 3.0, relation: EvidenceRelation::Contradicts }];
        let c = calculate_confidence(&ev);
        assert!(c < 0.5, "expected <0.5, got {c}");
    }

    #[test]
    fn mixed_evidence_cancels() {
        let ev = vec![
            ClaimEvidence { claim_id: 1, evidence_id: 1, strength: 2.0, relation: EvidenceRelation::Supports },
            ClaimEvidence { claim_id: 1, evidence_id: 2, strength: 2.0, relation: EvidenceRelation::Contradicts },
        ];
        let c = calculate_confidence(&ev);
        assert!((c - 0.5).abs() < 0.1, "expected ~0.5, got {c}");
    }
}
