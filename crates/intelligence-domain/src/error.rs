//! Intelligence domain errors.

use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum IntelligenceError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("infrastructure error: {0}")]
    Infrastructure(String),
}
