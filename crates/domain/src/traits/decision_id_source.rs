use async_trait::async_trait;

use crate::StoreError;

/// Allocates the next `decisions.id` for a brand-new decision aggregate.
///
/// The aggregate id's numeric suffix **is** the `decisions` primary key — P2
/// writes the row id explicitly from the aggregate id — and
/// `DecisionAggregate::propose` needs that id *before* it runs (its domain
/// events embed `DEC-{id}` at proposal time). So a fresh id must be allocated
/// up front by the layer that owns the id space (the store).
///
/// ## Consistency note
/// The D1 implementation reads `MAX(id) + 1` from `decisions` (single-writer
/// assumption). A cross-request create race is an accepted, documented risk —
/// consistent with SD-D's "no new transaction/UoW" boundary — and is tracked
/// as reliability backlog in the decision-vertical plan, not papered over.
#[async_trait(?Send)]
pub trait DecisionIdSource {
    /// Return the next decision id to assign to a new aggregate.
    async fn next_decision_id(&self) -> Result<i64, StoreError>;
}
