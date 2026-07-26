//! Read-model queries for the Outcome domain.
//!
//! Outcomes are created by [`super::super::repo::OutcomeRepository`].
//! These read methods were historically on `StoreBackend` as
//! `get_decision_outcomes`.

use async_trait::async_trait;

use crate::{OutcomeEvent, StoreError};

#[async_trait(?Send)]
pub trait OutcomeQueryService {
    /// List outcome observations for a decision.
    async fn list_outcomes(&self, decision_id: i64) -> Result<Vec<OutcomeEvent>, StoreError>;
}
