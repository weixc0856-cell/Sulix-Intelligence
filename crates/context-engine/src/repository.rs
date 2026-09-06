//! ContextRepository — persistence port for the context engine.
//!
//! Implementations live in `infrastructure` (e.g. `D1ContextRepository`).
//! Kept to the three methods the builder actually calls — not a general store
//! abstraction.

use async_trait::async_trait;

use crate::error::ContextError;
use crate::models::{DecisionRecord, MemoryRecord, NewContextSnapshot};

#[async_trait(?Send)]
pub trait ContextRepository {
    /// Decisions eligible as Advisor evidence — only rows whose status appears in
    /// `statuses`. Empty slice = no evidence. Implementations cap at `limit`.
    async fn list_decisions(&self, statuses: &[&str], limit: u32) -> Result<Vec<DecisionRecord>, ContextError>;
    async fn list_memories(&self, status: Option<&str>, limit: u32) -> Result<Vec<MemoryRecord>, ContextError>;
    async fn save_context_snapshot(&self, snap: &NewContextSnapshot) -> Result<(), ContextError>;
}
