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
/// This is deliberately **not** on the deprecated `StoreBackend` composite and
/// **not** a rename of the GATED `DecisionWriteStore` methods — it is the new
/// vertical's row write, added (P2, 2026-09-06) so
/// `infrastructure::D1DecisionRepository::save` can stop composing the legacy
/// `create_decision` + `update_decision_status` (which duplicated rows on a
/// second save and dropped `expected_outcomes`). `DecisionWriteStore` /
/// `StoreBackend` are deleted once the write path no longer references them.
#[async_trait(?Send)]
pub trait DecisionUpsertStore {
    /// Insert a decision row, or update it in place if the id already exists.
    async fn upsert_decision(&self, decision: &Decision) -> Result<(), StoreError>;
}
