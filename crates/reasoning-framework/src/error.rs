//! Reasoning Framework — error types

use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum FrameworkError {
    #[error("framework not found: {0}")]
    NotFound(String),
    #[error("invalid trigger rule: {0}")]
    InvalidTrigger(String),
    #[error("repository error: {0}")]
    Repository(String),
    #[error("seed error: {0}")]
    Seed(String),
}
