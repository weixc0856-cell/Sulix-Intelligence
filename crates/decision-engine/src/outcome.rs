//! Outcome value objects for the Decision aggregate.
//!
//! An ExpectedOutcome is a prediction made at decision time.
//! An ObservedOutcome is reality catching up with the prediction.
//! The gap between them is the core learning signal for Reflection.

use serde::{Deserialize, Serialize};

/// A predicted outcome established when the decision is proposed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedOutcome {
    pub metric: String,
    pub expected_value: String,
    pub measurement_method: String,
}

/// A real-world outcome observed after the decision was executed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedOutcome {
    pub metric: String,
    pub actual_value: String,
    pub outcome_type: OutcomeVerdict,
    pub evidence_url: Option<String>,
    pub observed_at: i64,
}

/// Whether an observed outcome met the expectation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeVerdict {
    Achieved,
    Missed,
    Inconclusive,
}
