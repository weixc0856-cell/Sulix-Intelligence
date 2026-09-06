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
/// application layer handles event emission separately (via
/// [`DecisionAggregate::drain_events`]).
///
/// ## `save` contract (P1, 2026-09-06)
///
/// `save(decision)` = **persist the aggregate's current state, keyed by
/// aggregate id**. The first `save` of an id inserts; later `save`s of
/// the same id update that row in place (idempotent in spirit — a second
/// `save` without state change is a no-op on the row). The concrete
/// mechanism an adapter chooses (D1 `INSERT … ON CONFLICT(id) DO UPDATE`,
/// which fields to refresh vs. preserve such as `created_at`) is an
/// **implementation detail** owned by the adapter — this contract must
/// not be specialised to D1, and callers may rely only on
/// "state is persisted and `find` returns it back afterwards". `save`
/// performs no invariant validation (the aggregate's behavioural methods
/// already did).
#[async_trait(?Send)]
pub trait DecisionRepository {
    /// Persist a decision aggregate's current state by aggregate id.
    async fn save(&self, decision: &DecisionAggregate) -> Result<(), DecisionError>;

    /// Persist a **brand-new** aggregate, atomically refusing to overwrite an
    /// id that already has a row.
    ///
    /// Returns `Ok(true)` when the new row was written; `Ok(false)` when a row
    /// with the same aggregate id already exists — a concurrent create claimed
    /// the id first (two creates can read the same `MAX(id)+1`; ADR-005) — and
    /// nothing was persisted (②, 2026-09-06). Only the create path calls this;
    /// lifecycle updates keep using [`save`], whose upsert semantics are correct
    /// for an aggregate that already owns its row.
    async fn save_new(&self, decision: &DecisionAggregate) -> Result<bool, DecisionError>;

    /// Load a decision aggregate by its domain ID.
    async fn find(&self, id: &str) -> Result<Option<DecisionAggregate>, DecisionError>;

    /// Find all decisions linked to a signal thread.
    async fn find_by_signal(&self, signal_thread_id: i64) -> Result<Vec<DecisionAggregate>, DecisionError>;

    /// List decisions, optionally filtered by status.
    async fn list(&self, status: Option<&str>, limit: u32) -> Result<Vec<DecisionAggregate>, DecisionError>;
}
