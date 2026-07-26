//! ConfidenceFactors — all factors that contribute to a confidence score.
//!
//! Each factor is normalized to 0.0–1.0 so the calculator can combine them
//! with a transparent geometric-mean formula rather than a black-box model.

/// Input factors for confidence calculation.
#[derive(Debug, Clone)]
pub struct ConfidenceFactors {
    /// Number of independent evidence items.
    pub evidence_count: u32,
    /// Aggregate quality/strength of evidence (0.0–1.0).
    pub evidence_strength: f64,
    /// Source reliability (0.0–1.0, mapped from Source.trust_score).
    pub source_trust: f64,
    /// Recency of evidence (0.0–1.0, 1.0 = today, decays over time).
    pub freshness: f64,
    /// Historical calibration accuracy for this entity type (0.0–1.0).
    pub calibration_score: f64,
}

impl ConfidenceFactors {
    /// All-zero factors (minimum confidence).
    pub fn zero() -> Self {
        Self { evidence_count: 0, evidence_strength: 0.0, source_trust: 0.0, freshness: 0.0, calibration_score: 0.0 }
    }
}

/// One factor's contribution to the final confidence, serialised for API output.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfidenceFactorExplanation {
    /// Machine-readable factor name, e.g. "evidence_strength", "source_trust".
    pub factor: String,
    /// Raw factor value after normalisation (0.0–1.0).
    pub value: f64,
    /// Weight this factor contributed to the geometric mean.
    pub weight: f64,
    /// Net impact on the final score (positive = raises confidence).
    pub impact: f64,
}

/// Result of a confidence calculation, including the decomposed factors.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfidenceResult {
    /// Final computed confidence (0.0–1.0).
    pub score: f64,
    /// Per-factor breakdown for explainability.
    pub factors: Vec<ConfidenceFactorExplanation>,
    /// Human-readable one-line reason.
    pub summary: String,
}
