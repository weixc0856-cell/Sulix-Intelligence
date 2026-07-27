//! Decision domain errors — pure domain, no infrastructure types.

use thiserror::Error;

use crate::status::DecisionStatus;

/// Errors produced by the Decision domain.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum DecisionError {
    #[error("invalid confidence value: {0}. Must be in [0, 1]")]
    InvalidConfidence(f64),

    #[error("decision title must not be empty")]
    EmptyTitle,

    #[error("invalid state transition: {from:?} → {to:?}")]
    InvalidTransition { from: DecisionStatus, to: DecisionStatus },

    #[error("decision must have at least one observed outcome before completing")]
    MissingOutcome,

    #[error("decision not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("infrastructure error: {0}")]
    Infrastructure(String),
}
