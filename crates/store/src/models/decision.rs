//! Decision Record — the Verifiable Decision Record model.
//!
//! A Decision is NOT a task. It is an Intelligence asset:
//! - Captures a hypothesis derived from Signal Thread evidence
//! - Records confidence in that hypothesis
//! - Tracks outcome verification over time
//!
//! Signal Thread → Decision → Outcome → Memory → Better Signal Evaluation

use serde::{Deserialize, Serialize};

/// Full decision record as stored in D1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: i64,
    pub signal_thread_id: Option<i64>,
    pub actor_id: Option<i64>,
    pub decision_type: String,
    pub title: String,
    pub hypothesis: Option<String>,
    pub rationale: Option<String>,
    pub confidence: f64,
    pub status: String,
    pub priority: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Input for creating a new decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDecision {
    pub signal_thread_id: Option<i64>,
    pub actor_id: Option<i64>,
    pub decision_type: String,
    pub title: String,
    pub hypothesis: Option<String>,
    pub rationale: Option<String>,
    pub confidence: f64,
    pub priority: String,
}

/// Lightweight summary for list views.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionSummary {
    pub id: i64,
    pub signal_thread_id: Option<i64>,
    pub decision_type: String,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub confidence: f64,
    pub created_at: i64,
}

impl From<Decision> for DecisionSummary {
    fn from(d: Decision) -> Self {
        Self {
            id: d.id,
            signal_thread_id: d.signal_thread_id,
            decision_type: d.decision_type,
            title: d.title,
            status: d.status,
            priority: d.priority,
            confidence: d.confidence,
            created_at: d.created_at,
        }
    }
}

// ===== Outcome Events =====

/// Outcome Observation — records what actually happened after a decision.
///
/// This is the **fact layer**: it only captures observations, not judgments.
/// Evaluation of whether the outcome confirms or contradicts the hypothesis
/// is handled separately by `DecisionEvaluation` (Sprint 3.3).
///
/// Multiple outcomes can be attached to a single decision over time,
/// forming a timeline of real-world observations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeEvent {
    pub id: i64,
    pub decision_id: i64,
    pub outcome_type: String,
    pub observation: String,
    pub evidence_url: Option<String>,
    pub observed_at: i64,
    pub created_at: i64,
}

/// Input for recording a new outcome observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewOutcomeEvent {
    pub decision_id: i64,
    pub outcome_type: String,
    pub observation: String,
    pub evidence_url: Option<String>,
    pub observed_at: Option<i64>,
}

// ===== Decision Evaluation (Sprint 3.3) =====

/// Fixed set of evaluation results — domain enum, not raw string.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationResult {
    Confirmed,
    PartiallyConfirmed,
    Contradicted,
    Inconclusive,
}

impl std::fmt::Display for EvaluationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Confirmed => write!(f, "confirmed"),
            Self::PartiallyConfirmed => write!(f, "partially_confirmed"),
            Self::Contradicted => write!(f, "contradicted"),
            Self::Inconclusive => write!(f, "inconclusive"),
        }
    }
}

impl From<&str> for EvaluationResult {
    fn from(s: &str) -> Self {
        match s {
            "confirmed" => Self::Confirmed,
            "partially_confirmed" => Self::PartiallyConfirmed,
            "contradicted" => Self::Contradicted,
            _ => Self::Inconclusive,
        }
    }
}

/// Who or what made the evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvaluationSource {
    Manual,
    AI,
}

impl std::fmt::Display for EvaluationSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Manual => write!(f, "manual"),
            Self::AI => write!(f, "ai"),
        }
    }
}

impl From<&str> for EvaluationSource {
    fn from(s: &str) -> Self {
        match s {
            "ai" => Self::AI,
            _ => Self::Manual,
        }
    }
}

/// A judgment about whether a decision's hypothesis was correct.
///
/// This is the **judgment layer**, separate from OutcomeObservation (fact layer).
/// Multiple evaluations can be made over time as new evidence arrives.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEvaluation {
    pub id: i64,
    pub decision_id: i64,
    pub evaluation: EvaluationResult,
    pub confidence: Option<f64>,
    pub reasoning: Option<String>,
    pub evaluator: EvaluationSource,
    pub evaluated_at: i64,
    pub created_at: i64,
}

/// Input for creating a new decision evaluation.
#[derive(Debug, Clone)]
pub struct NewDecisionEvaluation {
    pub decision_id: i64,
    pub evaluation: EvaluationResult,
    pub confidence: Option<f64>,
    pub reasoning: Option<String>,
    pub evaluator: EvaluationSource,
    pub evaluated_at: Option<i64>,
}
