//! Briefing application service — the D1 persistence face of the daily
//! briefing read use-cases (today / list / get) under `/api/intelligence/
//! briefing*`.
//!
//! Generic over the narrow [`store::BriefingStore`] seam.  The R2 Memory
//! Archive reads and KV caching that sit in front of this service are runtime
//! orchestration owned by `worker-entry`, NOT application logic (Phase 2 plan
//! §10), so they deliberately do not appear here.

use store::{BriefingSummary, StoreError};

/// Application service for the briefing persistence use-cases.
pub struct BriefingService<S> {
    store: S,
}

impl<S> BriefingService<S>
where
    S: store::BriefingStore,
{
    /// Wrap a store (or store-backed repository) in the service.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Load today's briefing content by its YYYY-MM-DD `date`; `None` if none
    /// was generated yet.  `date` is supplied by the caller — the application
    /// does not read the runtime clock.
    pub async fn today(&self, date: &str) -> Result<Option<String>, StoreError> {
        self.store.load_today_briefing(date).await
    }

    /// List available briefings, newest first.
    pub async fn list(&self) -> Result<Vec<BriefingSummary>, StoreError> {
        self.store.list_briefings().await
    }

    /// Get a briefing by its database id.
    pub async fn get(&self, id: i64) -> Result<Option<String>, StoreError> {
        self.store.get_briefing_by_id(id).await
    }
}
