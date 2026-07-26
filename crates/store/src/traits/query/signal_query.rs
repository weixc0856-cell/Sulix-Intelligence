//! Read-model queries for the Signal Intelligence domain.
//!
//! Signal thread mutations (`save_signal`, `find_signal*`) belong in
//! [`super::super::repo::SignalRepository`].  Instance appends and lifecycle
//! updates remain on [`StoreBackend`](crate::StoreBackend) until
//! event sourcing is formalised.

use async_trait::async_trait;

use std::collections::HashMap;

use crate::{
    BriefArticle, RadarResponse, RelatedEntityRef, SignalBriefInput, SignalDetail, SignalThreadFilter, StoreError,
    TodaySignal,
};

#[async_trait(?Send)]
pub trait SignalQueryService {
    /// Intelligence Radar dashboard — aggregated read model.
    async fn radar(&self, filter: &SignalThreadFilter) -> Result<RadarResponse, StoreError>;

    /// Full signal detail (thread info + timeline + evidence + entities).
    async fn signal_detail(&self, id: i64) -> Result<Option<SignalDetail>, StoreError>;

    /// List signal threads with dynamic filtering.
    async fn list_signal_threads(&self, filter: &SignalThreadFilter) -> Result<Vec<SignalBriefInput>, StoreError>;

    /// Get active signal threads (for briefing generation).
    async fn get_active_signal_threads(&self, limit: u32) -> Result<Vec<SignalBriefInput>, StoreError>;

    /// Legacy: today's signals (entity-anchored, V1 format).
    async fn signals_today(&self) -> Result<Vec<TodaySignal>, StoreError>;
}

/// Batch read-model queries for Radar / Projection.
///
/// Eliminates N+1 by loading all data in single batched SQL queries.
#[async_trait(?Send)]
pub trait BatchSignalQueryService {
    /// Batch-load evidence across multiple signal threads.
    /// Returns `HashMap<thread_id, Vec<BriefArticle>>`.
    async fn batch_evidence(&self, thread_ids: &[i64]) -> Result<HashMap<i64, Vec<BriefArticle>>, StoreError>;

    /// Batch-load related entities across multiple signal threads.
    /// Returns `HashMap<thread_id, Vec<RelatedEntityRef>>`.
    async fn batch_related_entities(
        &self,
        thread_ids: &[i64],
    ) -> Result<HashMap<i64, Vec<RelatedEntityRef>>, StoreError>;
}
