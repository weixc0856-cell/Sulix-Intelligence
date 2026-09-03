//! HTTP routes owned by the composition root (worker-entry).
//!
//! These are internal endpoints that the domain/application layer should not
//! know about — they are wiring only (build adapters, call application
//! services, map to HTTP). Route handlers must NOT copy business logic into
//! this crate; any logic stays in the domain/application crate it belongs to.

pub(crate) mod agent;
pub(crate) mod context;
mod response;
