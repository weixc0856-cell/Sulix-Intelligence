//! ReasoningTask — task-level routing for model requests.
//!
//! Different reasoning tasks may use different models, token budgets,
//! or prompt strategies. This enum allows the ModelProvider to apply
//! task-specific optimisations.

use serde::{Deserialize, Serialize};

/// Identifies the reasoning task for a model request.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReasoningTask {
    AgentAnswer,
    Reflection,
    Summarization,
    ClaimExtraction,
    SignalAnalysis,
}

impl std::fmt::Display for ReasoningTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AgentAnswer => write!(f, "agent"),
            Self::Reflection => write!(f, "reflection"),
            Self::Summarization => write!(f, "summarization"),
            Self::ClaimExtraction => write!(f, "claim_extraction"),
            Self::SignalAnalysis => write!(f, "signal_analysis"),
        }
    }
}
