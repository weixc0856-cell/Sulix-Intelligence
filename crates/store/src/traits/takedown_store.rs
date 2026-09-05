use async_trait::async_trait;

use crate::StoreError;

/// Takedown/compliance persistence seam (D1 `takedown_requests` +
/// `content_visibility_overrides`).
///
/// Added in Phase 2 so the `/api/compliance/*` use-cases ride a narrow port
/// instead of the concrete [`StoreBackend`](crate::StoreBackend) or inherent
/// `D1Store` methods.  The store returns loose row JSON for listings; no typed
/// aggregate model exists yet.
#[async_trait(?Send)]
pub trait TakedownStore {
    /// Submit a takedown request and block access to the referenced content.
    /// Returns the new takedown id.
    async fn create_takedown(
        &self,
        source_id: Option<i64>,
        article_id: Option<i64>,
        requester_email: &str,
        reason: &str,
    ) -> Result<i64, StoreError>;

    /// List takedown requests, optionally filtered by status.
    async fn list_takedowns(&self, status: Option<&str>, limit: u32) -> Result<Vec<serde_json::Value>, StoreError>;

    /// Update a takedown request's status (and optional reviewer notes).
    async fn update_takedown_status(&self, id: i64, status: &str, notes: Option<&str>) -> Result<(), StoreError>;
}
