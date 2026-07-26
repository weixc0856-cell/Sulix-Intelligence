use serde::{Deserialize, Serialize};

/// Status of a decision record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    Proposed,
    Active,
    Completed,
    Superseded,
}

/// Outcome status for a single metric.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeStatus {
    Pending,
    Achieved,
    Missed,
    Superseded,
}

/// Relationship between a decision and a claim.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaimRelationship {
    Supports,
    Contradicts,
    Context,
    Assumption,
}

/// A decision proposal generated from a signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionProposal {
    pub signal_id: i64,
    pub signal_title: String,
    pub recommended_action: String,
    pub alternatives: Vec<String>,
    pub rationale: String,
    pub confidence: f64,
    pub risks: Vec<String>,
    pub supporting_claims: Vec<ProposalClaimRef>,
}

/// A claim reference in a proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalClaimRef {
    pub claim_id: i64,
    pub claim_statement: String,
    pub relationship: String,
}

/// The 12-section Decision Memo artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionMemo {
    pub version: String,
    pub generated_at: i64,
    pub sections: Vec<MemoSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoSection {
    pub title: String,
    pub content: String,
    pub order: u32,
}

/// Decision domain events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum DecisionEvent {
    #[serde(rename = "decision.created")]
    DecisionCreated { decision_id: i64, title: String, confidence: f64, signal_id: Option<i64> },
    #[serde(rename = "outcome.recorded")]
    OutcomeRecorded { decision_id: i64, outcomes: Vec<OutcomeRecordedItem> },
    #[serde(rename = "reflection.completed")]
    ReflectionCompleted { decision_id: i64, reflection_id: i64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeRecordedItem {
    pub metric: String,
    pub expected: String,
    pub actual: String,
    pub achieved: bool,
}
