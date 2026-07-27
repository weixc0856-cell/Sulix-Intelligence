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
