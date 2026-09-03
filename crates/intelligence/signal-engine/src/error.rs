//! Signal Engine error type.
//!
//! The dependency-boundary ports (decoupling P3 Round 2) convert every
//! infrastructure error into this domain error, so the engine logic never
//! depends on a concrete adapter's error type.

/// Errors surfaced by the signal-engine domain ports.
#[derive(Debug, thiserror::Error)]
pub enum SignalError {
    #[error("signal persistence: {0}")]
    Persistence(String),
    #[error("signal discovery: {0}")]
    Discovery(String),
    #[error("signal query: {0}")]
    Query(String),
    #[error("signal event log: {0}")]
    EventLog(String),
    #[error("semantic index: {0}")]
    Semantic(String),
}
