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
