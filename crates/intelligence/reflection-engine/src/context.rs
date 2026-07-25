//! Reflection context — immutable snapshot of what was decided, why, and what happened.
//!
//! The [`ReflectionContextBuilder`] loads data from D1 and computes a
//! completeness score.  The engine uses this context to generate the
//! reflection via LLM.

use store::StoreBackend;

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

/// Builds a `ReflectionContext` by loading data from the store.
///
/// Formula for completeness_score:
///   decision_exists * 0.3 + thesis_exists * 0.2 + outcome_exists * 0.3 + evidence_exists * 0.2
pub struct ReflectionContextBuilder<'a, S: StoreBackend> {
    store: &'a S,
}

impl<'a, S: StoreBackend> ReflectionContextBuilder<'a, S> {
    pub fn new(store: &'a S) -> Self {
        Self { store }
    }

    /// Load all context for a decision.
    pub async fn build(&self, decision_id: i64) -> Result<ReflectionContext, store::StoreError> {
        let decision = self.store.get_decision(decision_id).await?;
        let outcomes = self.store.get_decision_outcomes(decision_id).await?;
        let evaluations = self.store.get_decision_evaluations(decision_id).await?;

        let (decision_snap, thesis_snap) = match decision {
            Some(d) => (
                DecisionSnapshot {
                    id: d.id,
                    title: d.title.clone(),
                    decision_type: d.decision_type.clone(),
                },
                ThesisSnapshot {
                    hypothesis: d.hypothesis.unwrap_or_default(),
                    assumptions: Vec::new(),
                    initial_confidence: d.confidence,
                },
            ),
            None => return Err(store::StoreError::D1("decision not found".into())),
        };

        let outcome_snap = outcomes.first().map(|o| OutcomeSnapshot {
            id: o.id,
            outcome_type: o.outcome_type.clone(),
            observation: o.observation.clone(),
        });

        let eval_snaps: Vec<EvaluationSnapshot> = evaluations
            .into_iter()
            .map(|e| EvaluationSnapshot {
                evaluation: e.evaluation.to_string(),
                confidence: e.confidence,
                reasoning: e.reasoning,
            })
            .collect();

        // Compute completeness score
        let decision_score = 0.3;
        let thesis_score = if thesis_snap.hypothesis.is_empty() { 0.0 } else { 0.2 };
        let outcome_score = if outcome_snap.is_some() { 0.3 } else { 0.0 };
        let evidence_score = 0.2; // placeholder — could check signal evidence
        let completeness = decision_score + thesis_score + outcome_score + evidence_score;

        Ok(ReflectionContext {
            decision: decision_snap,
            thesis: thesis_snap,
            outcome: outcome_snap,
            evaluations: eval_snaps,
            evidence: Vec::new(),
            completeness_score: completeness,
        })
    }
}
