use async_trait::async_trait;

use crate::{NewOutcomeEvent, StoreError};

/// Outcome aggregate persistence.
///
/// Outcome is a separate aggregate root, linked to Decision by `decision_id`.
/// Decisions do NOT own outcomes — they reference them.
#[async_trait(?Send)]
pub trait OutcomeRepository {
    /// Record a factual outcome observation.  Returns the outcome id.
    async fn save_outcome(&self, e: &NewOutcomeEvent) -> Result<i64, StoreError>;
}
