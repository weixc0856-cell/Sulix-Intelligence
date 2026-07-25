//! Briefing Context — Intelligence state for decision-aware LLM analysis.
//!
//! Carries entity, decision, and evaluation context alongside each signal
//! so the LLM can produce analysis that respects historical commitments
//! and prior judgments — not just raw article summaries.

/// Entity context — who is mentioned alongside this signal.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EntityContext {
    pub name: String,
    pub entity_type: String,
    pub relevance: f64,
}

/// Decision context — what decisions track this signal.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DecisionContext {
    pub id: i64,
    pub title: String,
    pub status: String,
    pub latest_evaluation: Option<String>,
}

/// Full briefing context for a single signal.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BriefingContext {
    pub entities: Vec<EntityContext>,
    pub decisions: Vec<DecisionContext>,
}
