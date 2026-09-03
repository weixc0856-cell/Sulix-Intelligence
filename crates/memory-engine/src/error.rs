//! Memory bounded context — domain errors.

/// Errors surfaced by the Memory bounded context.
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    /// A persistence / outbox failure surfaced through the repository port.
    #[error("persistence error: {0}")]
    Persistence(String),
}
