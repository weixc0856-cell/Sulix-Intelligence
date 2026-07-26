//! Shared domain error type.
//!
//! Every domain crate converts its specific errors into `DomainError`
//! at its boundary, so application services can handle failures uniformly.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("validation: {0}")]
    Validation(String),

    #[error("invalid state transition: {0}")]
    InvalidTransition(String),

    #[error("persistence: {0}")]
    Persistence(String),

    #[error("serialization: {0}")]
    Serialization(String),
}
