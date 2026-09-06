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

/// Encode a list of [`ExpectedOutcome`] as a JSON array string for the D1
/// `expected_outcomes` TEXT column. Infallible — [`ExpectedOutcome`] is a
/// plain serde struct, so serialization cannot fail.
///
/// Pure domain helper (P1, 2026-09-06): the adapter owns *where* the
/// string is stored; this owns *how* an `[ExpectedOutcome]` becomes JSON.
pub fn encode_expected_outcomes(outcomes: &[ExpectedOutcome]) -> String {
    serde_json::to_string(outcomes).expect("ExpectedOutcome is always serializable")
}

/// Decode the `expected_outcomes` D1 column (a JSON array string) back
/// into [`ExpectedOutcome`]s. Tolerant by contract: `None`, empty, or
/// malformed JSON all degrade to `Vec::new()` so rows written before
/// migration 0050 (column `NULL`) hydrate cleanly rather than erroring.
pub fn decode_expected_outcomes(column: Option<&str>) -> Vec<ExpectedOutcome> {
    match column {
        None => Vec::new(),
        Some(raw) => serde_json::from_str(raw).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_outcomes() -> Vec<ExpectedOutcome> {
        vec![
            ExpectedOutcome {
                metric: "accuracy".into(),
                expected_value: ">= 0.9".into(),
                measurement_method: "eval set".into(),
            },
            ExpectedOutcome {
                metric: "latency".into(),
                expected_value: "< 200ms".into(),
                measurement_method: "p95".into(),
            },
        ]
    }

    #[test]
    fn encode_then_decode_round_trips() {
        let encoded = encode_expected_outcomes(&sample_outcomes());
        let decoded = decode_expected_outcomes(Some(&encoded));
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].metric, "accuracy");
        assert_eq!(decoded[0].expected_value, ">= 0.9");
        assert_eq!(decoded[1].metric, "latency");
        assert_eq!(decoded[1].expected_value, "< 200ms");
        assert_eq!(decoded[1].measurement_method, "p95");
    }

    #[test]
    fn empty_slice_encodes_as_empty_array_and_decodes_back() {
        let encoded = encode_expected_outcomes(&[]);
        assert_eq!(encoded, "[]");
        assert!(decode_expected_outcomes(Some(&encoded)).is_empty());
    }

    #[test]
    fn decode_degrades_null_empty_whitespace_and_malformed_to_empty() {
        assert!(decode_expected_outcomes(None).is_empty());
        assert!(decode_expected_outcomes(Some("")).is_empty());
        // Whitespace is not valid JSON → degrade, not error.
        assert!(decode_expected_outcomes(Some("  ")).is_empty());
        assert!(decode_expected_outcomes(Some("not json")).is_empty());
        // Well-formed JSON of the wrong shape → degrade, not error.
        assert!(decode_expected_outcomes(Some("[1, 2]")).is_empty());
        assert!(decode_expected_outcomes(Some("{}")).is_empty());
    }
}
