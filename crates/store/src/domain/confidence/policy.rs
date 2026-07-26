//! ConfidencePolicy — calculation configuration and bounds.
//!
//! All tunable parameters are centralised here so they can be adjusted
//! without touching the calculation logic.

/// Configuration for confidence calculation.
#[derive(Debug, Clone)]
pub struct ConfidencePolicy {
    /// Floor — confidence never goes below this (prevents 0).
    pub min_confidence: f64,
    /// Ceiling — confidence never goes above this (prevents false certainty).
    pub max_confidence: f64,
    /// Number of evidence items at which diminishing returns saturate.
    pub evidence_saturation: u32,
}

impl Default for ConfidencePolicy {
    fn default() -> Self {
        Self { min_confidence: 0.05, max_confidence: 0.98, evidence_saturation: 10 }
    }
}
