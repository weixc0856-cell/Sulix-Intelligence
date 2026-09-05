//! Compliance application service — the takedown use-cases (submit / list /
//! update-status) under `/api/compliance/*`.
//!
//! Generic over the narrow [`store::TakedownStore`] seam.  Request-shape
//! validation (field presence, id/status formats) stays in the route layer;
//! this service owns the persistence orchestration the routes delegate to.

use store::StoreError;

/// Application service for the takedown/compliance use-cases.
pub struct ComplianceService<S> {
    store: S,
}

impl<S> ComplianceService<S>
where
    S: store::TakedownStore,
{
    /// Wrap a store (or store-backed repository) in the service.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Submit a takedown request against a source and/or article; returns the
    /// new takedown id.
    pub async fn submit(
        &self,
        source_id: Option<i64>,
        article_id: Option<i64>,
        requester_email: &str,
        reason: &str,
    ) -> Result<i64, StoreError> {
        self.store.create_takedown(source_id, article_id, requester_email, reason).await
    }

    /// List takedown requests, optionally filtered by status.
    pub async fn list(&self, status_filter: Option<&str>, limit: u32) -> Result<Vec<serde_json::Value>, StoreError> {
        self.store.list_takedowns(status_filter, limit).await
    }

    /// Update a takedown request's status and optional reviewer notes.
    pub async fn update_status(&self, id: i64, status: &str, notes: Option<&str>) -> Result<(), StoreError> {
        self.store.update_takedown_status(id, status, notes).await
    }
}
