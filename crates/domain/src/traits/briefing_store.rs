use async_trait::async_trait;

use crate::{BriefingSummary, StoreError};

/// Daily-briefing persistence seam (D1 `intelligence_briefs`).
///
/// Added in Phase 2 so the briefing use-cases ride a narrow port instead of
/// the concrete [`StoreBackend`](crate::StoreBackend) or inherent `D1Store`
/// methods.  R2 archive reads and KV caching are deliberately NOT here — they
/// are runtime orchestration owned by `worker-entry` (Phase 2 plan §10).
#[async_trait(?Send)]
pub trait BriefingStore {
    /// Persist a generated daily briefing (upsert by date).
    async fn save_briefing(
        &self,
        date: &str,
        generated_at: i64,
        signal_count: u32,
        content: &str,
    ) -> Result<(), StoreError>;

    /// Load the briefing whose `date` column matches; `None` if none was
    /// generated yet.
    async fn load_today_briefing(&self, date: &str) -> Result<Option<String>, StoreError>;

    /// List available briefings, newest first.
    async fn list_briefings(&self) -> Result<Vec<BriefingSummary>, StoreError>;

    /// Get a briefing by its database id.
    async fn get_briefing_by_id(&self, id: i64) -> Result<Option<String>, StoreError>;
}
