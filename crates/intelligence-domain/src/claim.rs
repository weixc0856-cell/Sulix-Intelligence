//! Claim domain — atomic, falsifiable judgments extracted from evidence.

use serde::{Deserialize, Serialize};

/// Type of claim — determines how confidence is evaluated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaimType {
    Fact,
    Trend,
    Prediction,
    Causal,
    Opinion,
}

/// A claim — an atomic falsifiable judgment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    pub id: i64,
    pub statement: String,
    pub claim_type: ClaimType,
    pub confidence: Option<f64>,
    pub status: String,
    pub article_id: Option<i64>,
    pub created_at: i64,
}

/// Input for creating a new claim.
#[derive(Debug, Clone)]
pub struct NewClaim {
    pub statement: String,
    pub claim_type: ClaimType,
    pub article_id: Option<i64>,
}

/// Reference from a claim to supporting evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub article_id: i64,
    pub relevance: f64,
    pub relation: EvidenceRelation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRelation {
    Supports,
    Contradicts,
    Weakens,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_serde_roundtrip() {
        let claim = Claim {
            id: 1,
            statement: "AI adoption accelerates".into(),
            claim_type: ClaimType::Trend,
            confidence: Some(0.82),
            status: "active".into(),
            article_id: Some(42),
            created_at: 1000,
        };
        let json = serde_json::to_string(&claim).unwrap();
        let parsed: Claim = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.statement, "AI adoption accelerates");
        assert_eq!(parsed.claim_type, ClaimType::Trend);
        assert_eq!(parsed.confidence, Some(0.82));
    }

    #[test]
    fn claim_types_serde() {
        for ct in &[ClaimType::Fact, ClaimType::Trend, ClaimType::Prediction, ClaimType::Causal, ClaimType::Opinion] {
            let json = serde_json::to_string(ct).unwrap();
            let parsed: ClaimType = serde_json::from_str(&json).unwrap();
            assert_eq!(*ct, parsed);
        }
    }

    #[test]
    fn evidence_relation_serde() {
        let json = serde_json::to_string(&EvidenceRelation::Supports).unwrap();
        assert_eq!(json, "\"supports\"");
        let parsed: EvidenceRelation = serde_json::from_str("\"contradicts\"").unwrap();
        assert_eq!(parsed, EvidenceRelation::Contradicts);
    }
}
