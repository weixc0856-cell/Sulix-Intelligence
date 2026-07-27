//! Domain types for claim extraction and management.

use serde::{Deserialize, Serialize};

/// Type of claim — determines how confidence is evaluated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaimType {
    /// Verifiable, specific, about past/present.
    Fact,
    /// Directional change over time.
    Trend,
    /// Future outcome.
    Prediction,
    /// X causes Y.
    Causal,
    /// Value judgment or interpretation.
    Opinion,
}

/// Level of uncertainty in a claim.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Uncertainty {
    Low,
    Medium,
    High,
}

/// A reference from a claim to an evidence article.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub article_id: i64,
    pub relevance: f64,
}

/// A reference to a reasoning framework applied to a claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameworkRef {
    pub framework_id: String,
    pub relevance: f64,
    pub reasoning: String,
}

/// A candidate claim extracted from an article by the LLM.
/// No confidence field — confidence is computed by ConfidenceEngine v2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimCandidate {
    /// The claim statement.
    pub statement: String,
    /// Type of claim.
    pub claim_type: ClaimType,
    /// Why the evidence supports this claim.
    pub reasoning: String,
    /// What would prove this claim wrong.
    pub falsification: String,
    /// References to supporting articles.
    pub evidence_refs: Vec<EvidenceRef>,
    /// Counter-arguments to this claim.
    pub counter_arguments: Vec<String>,
    /// Reasoning frameworks applied to this claim.
    pub frameworks_applied: Vec<FrameworkRef>,
    /// LLM's assessment of uncertainty (NOT confidence score).
    pub uncertainty: Uncertainty,
}

/// LLM output shape for claim extraction.
#[derive(Debug, Deserialize)]
pub(crate) struct LlmClaimOutput {
    pub claims: Vec<LlmClaimItem>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LlmClaimItem {
    pub claim_type: String,
    pub statement: String,
    pub reasoning: String,
    #[serde(default)]
    pub falsification: String,
    #[serde(default)]
    pub evidence_article_ids: Vec<i64>,
    #[serde(default)]
    pub counter_arguments: Vec<String>,
    #[serde(default)]
    pub frameworks_applied: Vec<FrameworkRef>,
    #[serde(default)]
    pub uncertainty: String,
}
