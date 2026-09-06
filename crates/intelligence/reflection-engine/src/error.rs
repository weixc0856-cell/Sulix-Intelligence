//! Reflection bounded context — domain errors.

/// Errors surfaced by the Reflection bounded context.
///
/// Kept here (not in `store`) so domain code and the repository port share one
/// error vocabulary. The infrastructure adapter maps persistence failures into
/// [`ReflectionError::Persistence`].
#[derive(Debug, thiserror::Error)]
pub enum ReflectionError {
    /// A persistence / outbox failure surfaced through the repository port.
    #[error("persistence error: {0}")]
    Persistence(String),

    /// The decision being reflected on does not exist.
    #[error("decision {0} not found")]
    DecisionNotFound(i64),

    /// A reflection row already exists for this decision and is not eligible to
    /// be re-opened for a retry (it is `generated`/`generating`, or its `failed`
    /// row has exhausted the retry cap). `UNIQUE(decision_id)` allows at most one
    /// row per decision, so a duplicate invocation must not disturb the row.
    #[error("reflection already tracked for decision {0}")]
    AlreadyTracked(i64),
}
