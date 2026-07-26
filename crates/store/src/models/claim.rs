use serde::{Deserialize, Serialize};

/// Claim — an atomic, falsifiable judgment extracted from evidence.
///
/// Sprint 5.9C: Claim is immutable (no confidence field).
/// Confidence is tracked via ConfidenceEvent (entity_type: "claim").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    pub id: i64,
    pub statement: String,
    pub claim_type: String, // "fact" | "trend" | "prediction" | "causal" | "opinion"
    pub reasoning: Option<String>,
    pub falsification: Option<String>,
    pub status: String,
    pub article_id: Option<i64>,
    pub observation_id: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Input for creating a new Claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewClaim {
    pub statement: String,
    pub claim_type: String,
    pub reasoning: Option<String>,
    pub falsification: Option<String>,
    pub status: Option<String>,
    pub article_id: Option<i64>,
    pub observation_id: Option<i64>,
}

/// Evidence linking a claim to an article.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimEvidence {
    pub claim_id: i64,
    pub article_id: i64,
    pub relation: String, // "supports" | "contradicts" | "weakens"
    pub strength: f64,
    pub created_at: i64,
}

/// ArticleEvidence — query DTO that JOINs claim_evidence with articles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleEvidence {
    pub claim_id: i64,
    pub article_id: i64,
    pub article_title: String,
    pub relation: String,
    pub strength: f64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvidenceRelation {
    Supports,
    Contradicts,
    Weakens,
}

impl EvidenceRelation {
    pub fn as_str(&self) -> &'static str {
        match self {
            EvidenceRelation::Supports => "supports",
            EvidenceRelation::Contradicts => "contradicts",
            EvidenceRelation::Weakens => "weakens",
        }
    }
}
