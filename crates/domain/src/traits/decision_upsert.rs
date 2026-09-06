use async_trait::async_trait;

use crate::{Decision, StoreError};

/// Canonical decision-row write port for the decision-engine vertical.
///
/// `save(aggregate)` (see `decision_engine::DecisionRepository`) persists the
/// aggregate's **current state by aggregate id**; the concrete adapter maps the
/// aggregate onto a [`Decision`] row and persists it here. The store is
/// responsible for the idempotent insert-or-update semantics:
///
/// - the row's primary key (`decisions.id`) makes the id unique — a second
///   `upsert_decision` of the same id updates that row in place, never
///   duplicates it;
/// - aggregate-owned columns (title, hypothesis, status, expected_outcomes,
///   …) are refreshed on update;
/// - `created_at` is preserved from the first insert (never rewritten by a
///   later upsert) and `updated_at` is refreshed.
///
/// This is the decision-engine vertical's canonical row write (P2, 2026-09-06),
/// added so `infrastructure::D1DecisionRepository::save` persists an aggregate
/// state in place. It replaced the legacy two-step row insert + separate status
/// update, which duplicated rows on a second save and dropped
/// `expected_outcomes`.
#[async_trait(?Send)]
pub trait DecisionUpsertStore {
    /// Insert a decision row, or update it in place if the id already exists.
    async fn upsert_decision(&self, decision: &Decision) -> Result<(), StoreError>;

    /// Insert a **brand-new** decision row, refusing (not overwriting) when the
    /// id already exists.
    ///
    /// Returns `Ok(true)` when the row was inserted, `Ok(false)` when a row with
    /// the same id is already present and nothing was written — i.e. a racing
    /// create claimed the id first (②, 2026-09-06; ADR-005).
    ///
    /// The create path must use this instead of [`upsert_decision`], whose
    /// `ON CONFLICT(id) DO UPDATE` would otherwise let a second concurrent
    /// create (both having read the same `MAX(id)+1`) silently overwrite the
    /// first creator's row. Update paths (persisting an existing aggregate) keep
    /// using [`upsert_decision`].
    async fn try_insert_decision(&self, decision: &Decision) -> Result<bool, StoreError>;
}
