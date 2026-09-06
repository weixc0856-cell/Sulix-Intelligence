use async_trait::async_trait;

use crate::{Decision, StoreError};

/// DTO read access to the decision row.
///
/// Writes of the decision row go through `DecisionUpsertStore`
/// (decision-engine vertical); this trait only reads. Outcomes and evaluations
/// have their own narrow write ports. Read-model queries (stats, outcomes,
/// evaluations) belong in [`super::super::query::DecisionQueryService`].
#[async_trait(?Send)]
pub trait DecisionRepository {
    /// Load a decision by its primary key.
    async fn find_decision(&self, id: i64) -> Result<Option<Decision>, StoreError>;
}
