//! Error type for the context-engine persistence ports.

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("context persistence error: {0}")]
    Persistence(String),
}
