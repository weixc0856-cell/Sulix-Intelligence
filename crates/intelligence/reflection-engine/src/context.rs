//! Reflection context — immutable snapshot of what was decided, why, and what happened.
//!
//! The [`ReflectionContextBuilder`] pulls the decision facts through the
//! domain-owned [`ReflectionRepository`] port and computes a completeness
//! score.  The engine uses this context to generate the reflection via LLM.
//!
//! D1 row→snapshot mapping happens in the infrastructure adapter
//! (`load_decision_context`), so this module only ever sees domain value
//! objects.

use crate::error::ReflectionError;
use crate::repository::ReflectionRepository;

/// Snapshot of the decision itself.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DecisionSnapshot {
    pub id: i64,
    pub title: String,
    pub decision_type: String,
}

/// The original thesis — what was believed at decision time.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThesisSnapshot {
    pub hypothesis: String,
    pub assumptions: Vec<String>,
    pub initial_confidence: f64,
}

/// Snapshot of the outcome (the result of the decision).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OutcomeSnapshot {
    pub id: i64,
    pub outcome_type: String,
    pub observation: String,
}

/// Snapshot of an evaluation judgment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EvaluationSnapshot {
    pub evaluation: String,
    pub confidence: Option<f64>,
    pub reasoning: Option<String>,
}

/// An evidence item that informed the decision.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EvidenceItem {
    pub source: String,
    pub summary: String,
    pub relevance_score: f64,
    pub captured_at: i64,
}

/// Full context for generating a reflection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReflectionContext {
    pub decision: DecisionSnapshot,
    pub thesis: ThesisSnapshot,
    pub outcome: Option<OutcomeSnapshot>,
    pub evaluations: Vec<EvaluationSnapshot>,
    pub evidence: Vec<EvidenceItem>,
    pub completeness_score: f64,
}

/// Builds a `ReflectionContext` from the decision facts the repository loads.
///
/// Formula for completeness_score:
///   decision_exists * 0.3 + thesis_exists * 0.2 + outcome_exists * 0.3 + evidence_exists * 0.2
pub struct ReflectionContextBuilder<'a, R: ReflectionRepository> {
    repository: &'a R,
}

impl<'a, R: ReflectionRepository> ReflectionContextBuilder<'a, R> {
    pub fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    /// Load all context for a decision.
    pub async fn build(&self, decision_id: i64) -> Result<ReflectionContext, ReflectionError> {
        let facts = self
            .repository
            .load_decision_context(decision_id)
            .await?
            .ok_or(ReflectionError::DecisionNotFound(decision_id))?;

        let decision_snap =
            DecisionSnapshot { id: facts.decision_id, title: facts.title, decision_type: facts.decision_type };

        let thesis_snap = ThesisSnapshot {
            hypothesis: facts.hypothesis.unwrap_or_default(),
            assumptions: Vec::new(),
            initial_confidence: facts.confidence,
        };

        // Compute completeness score
        let decision_score = 0.3;
        let thesis_score = if thesis_snap.hypothesis.is_empty() { 0.0 } else { 0.2 };
        let outcome_score = if facts.outcome.is_some() { 0.3 } else { 0.0 };
        let evidence_score = 0.2; // placeholder — could check signal evidence
        let completeness = decision_score + thesis_score + outcome_score + evidence_score;

        Ok(ReflectionContext {
            decision: decision_snap,
            thesis: thesis_snap,
            outcome: facts.outcome,
            evaluations: facts.evaluations,
            evidence: Vec::new(),
            completeness_score: completeness,
        })
    }
}
