//! Domain-owned repository trait for the Decision aggregate.
//!
//! Defined here (not in `store`) so the domain depends on nothing
//! infrastructure-specific. The concrete `D1DecisionRepository` lives
//! in `crates/infrastructure/d1/`.

use async_trait::async_trait;

use crate::aggregate::DecisionAggregate;
use crate::error::DecisionError;

/// Repository for [`DecisionAggregate`] persistence.
///
/// Methods use domain types exclusively — no D1, no JsValue, no SQL.
/// The `save` method persists the aggregate's current state; the
/// application layer handles event emission separately.
#[async_trait(?Send)]
pub trait DecisionRepository {
    /// Persist a decision aggregate (insert or update).
    async fn save(&self, decision: &DecisionAggregate) -> Result<(), DecisionError>;

    /// Load a decision aggregate by its domain ID.
    async fn find(&self, id: &str) -> Result<Option<DecisionAggregate>, DecisionError>;

    /// Find all decisions linked to a signal thread.
    async fn find_by_signal(&self, signal_thread_id: i64) -> Result<Vec<DecisionAggregate>, DecisionError>;

    /// List decisions, optionally filtered by status.
    async fn list(&self, status: Option<&str>, limit: u32) -> Result<Vec<DecisionAggregate>, DecisionError>;
}
