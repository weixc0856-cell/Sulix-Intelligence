//! ConfidenceCalculator — transparent, factor-based confidence scoring.
//!
//! Formula (v1 — geometric mean of weighted factors):
//!
//!   confidence = (evidence_contrib × source_trust × freshness × calibration)^(1/4)
//!
//! Each factor contributes equally in the geometric mean, making the score
//! sensitive to the *weakest* factor — if any factor is near zero the
//! confidence drops significantly, which is the correct property for a
//! trust-based system.
//!
//! ## Factor derivation
//!
//! | Factor              | Derivation                                      |
//! |---------------------|-------------------------------------------------|
//! | evidence_contrib    | evidence_strength × (1 − e^(−count / saturation)) |
//! | source_trust        | Source.trust_score (0.0–1.0)                    |
//! | freshness           | max(0, 1 − days_since_evidence / halflife_90d) |
//! | calibration         | historical_prediction_accuracy                  |

use crate::domain::confidence::factors::{ConfidenceFactorExplanation, ConfidenceFactors, ConfidenceResult};
use crate::domain::confidence::policy::ConfidencePolicy;

/// Compute confidence from factors using an interpretable geometric-mean formula.
pub fn calculate(factors: &ConfidenceFactors, policy: &ConfidencePolicy) -> ConfidenceResult {
    // 1. Evidence contribution with diminishing returns
    let evidence_contrib = factors.evidence_strength
        * (1.0 - (-(factors.evidence_count as f64) / policy.evidence_saturation as f64).exp());

    // 2. Clamp all factors to [0, 1]
    let ev = evidence_contrib.clamp(0.0, 1.0);
    let st = factors.source_trust.clamp(0.0, 1.0);
    let fr = factors.freshness.clamp(0.0, 1.0);
    let ca = factors.calibration_score.clamp(0.0, 1.0);

    // 3. Geometric mean (equal weights w = 0.25 each)
    let raw = (ev * st * fr * ca).powf(0.25);

    // 4. Apply policy bounds
    let score = raw.clamp(policy.min_confidence, policy.max_confidence);

    // 5. Build factor explanations
    let factors_expl = vec![
        explain("evidence_strength", ev, 0.25, score),
        explain("source_trust", st, 0.25, score),
        explain("freshness", fr, 0.25, score),
        explain("calibration", ca, 0.25, score),
    ];

    // 6. Generate a human-readable summary
    let summary = summarize(ev, st, fr, ca, score);

    ConfidenceResult { score, factors: factors_expl, summary }
}

/// Compute one factor's impact on the final score.
fn explain(name: &str, value: f64, weight: f64, _final_score: f64) -> ConfidenceFactorExplanation {
    // Impact = how much this factor's current value *raises* the final score
    // relative to a baseline of 0.5 (neutral).
    let baseline = 0.5;
    let delta = value - baseline;
    let impact = delta * weight * 2.0; // scale to roughly ±0.25 range

    ConfidenceFactorExplanation { factor: name.to_string(), value, weight, impact: impact.clamp(-0.5, 0.5) }
}

/// Build a one-line summary of the confidence drivers.
fn summarize(evidence: f64, source: f64, freshness: f64, calibration: f64, score: f64) -> String {
    let mut drivers: Vec<&str> = Vec::new();

    if evidence > 0.6 {
        drivers.push("strong evidence");
    } else if evidence < 0.3 {
        drivers.push("limited evidence");
    }

    if source > 0.7 {
        drivers.push("high-quality source");
    } else if source < 0.4 {
        drivers.push("low-trust source");
    }

    if freshness < 0.3 {
        drivers.push("stale data");
    }

    if calibration > 0.7 {
        drivers.push("good historical accuracy");
    }

    if drivers.is_empty() {
        format!("Confidence {:.0}% — neutral factors", score * 100.0)
    } else {
        format!("Confidence {:.0}% — {}", score * 100.0, drivers.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::confidence::factors::ConfidenceFactors;
    use crate::domain::confidence::policy::ConfidencePolicy;

    #[test]
    fn high_evidence_high_trust() {
        // Use high count to saturate diminishing returns
        let factors = ConfidenceFactors {
            evidence_count: 30,
            evidence_strength: 0.9,
            source_trust: 0.85,
            freshness: 1.0,
            calibration_score: 0.8,
        };
        let policy = ConfidencePolicy::default();
        let result = calculate(&factors, &policy);
        assert!(result.score > 0.7, "score should be high: {:.3}", result.score);
        assert_eq!(result.factors.len(), 4);
        assert!(result.summary.contains("strong evidence"), "summary: {}", result.summary);
        assert!(result.summary.contains("high-quality source"), "summary: {}", result.summary);
    }

    #[test]
    fn low_evidence_low_confidence() {
        let factors = ConfidenceFactors {
            evidence_count: 1,
            evidence_strength: 0.3,
            source_trust: 0.4,
            freshness: 0.2,
            calibration_score: 0.5,
        };
        let policy = ConfidencePolicy::default();
        let result = calculate(&factors, &policy);
        assert!(result.score < 0.5, "score should be low: {:.3}", result.score);
    }

    #[test]
    fn zero_evidence_minimum() {
        let factors = ConfidenceFactors::zero();
        let policy = ConfidencePolicy::default();
        let result = calculate(&factors, &policy);
        assert!(
            (result.score - policy.min_confidence).abs() < 0.01,
            "zero factors should floor at min_confidence: {:.3}",
            result.score
        );
    }

    #[test]
    fn four_factors_returned() {
        let factors = ConfidenceFactors {
            evidence_count: 5,
            evidence_strength: 0.8,
            source_trust: 0.7,
            freshness: 0.9,
            calibration_score: 0.6,
        };
        let result = calculate(&factors, &ConfidencePolicy::default());
        assert_eq!(result.factors.len(), 4);
        for f in &result.factors {
            assert!(f.impact >= -0.5 && f.impact <= 0.5, "impact out of range for {}: {:.3}", f.factor, f.impact);
        }
    }

    #[test]
    fn stale_data_penalty() {
        let fresh = calculate(&ConfidenceFactors { freshness: 1.0, ..high_factors() }, &ConfidencePolicy::default());
        let stale = calculate(&ConfidenceFactors { freshness: 0.1, ..high_factors() }, &ConfidencePolicy::default());
        assert!(stale.score < fresh.score, "stale data should reduce confidence");
    }

    fn high_factors() -> ConfidenceFactors {
        ConfidenceFactors {
            evidence_count: 30,
            evidence_strength: 0.9,
            source_trust: 0.85,
            freshness: 1.0,
            calibration_score: 0.8,
        }
    }
}
