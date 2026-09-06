use async_trait::async_trait;

use crate::{DiscoveryMethod, EntitySignalCandidate, SignalDetail, SignalEvent, SignalUpsertResult, StoreError};

/// Signal lifecycle + instance + timeline-event persistence and read-back.
///
/// Infra adapters and event-store backends bind this narrow seam directly.
/// Signal thread aggregates (`save_signal`/`find_signal*`) belong in
/// [`super::repo::SignalRepository`]; read-model queries (radar, detail,
/// listing) belong in [`super::query::SignalQueryService`]. This trait holds
/// the pre-Event-Sourcing instance/event/candidate slice until event sourcing
/// is formalised.
#[async_trait(?Send)]
pub trait SignalStore {
    /// Upsert a signal thread by its `signal_key`, returning the thread id
    /// and whether it was created or updated.
    async fn upsert_signal_thread(
        &self,
        signal_key: &str,
        anchor_entity_id: Option<i64>,
        title: &str,
        status: &str,
        discovery_method: &DiscoveryMethod,
        discovery_score: Option<f64>,
    ) -> Result<SignalUpsertResult, StoreError>;

    /// Update signal lifecycle (active → decaying → resolved → archived).
    async fn update_signal_lifecycle(&self, now: i64) -> Result<(), StoreError>;

    /// Load full signal detail (thread info + timeline + evidence + entities).
    async fn load_signal_detail(&self, thread_id: i64) -> Result<Option<SignalDetail>, StoreError>;

    /// Get the latest instance's (score, trend) for dedup.
    async fn get_latest_instance_fingerprint(&self, thread_id: i64) -> Result<Option<(f64, String)>, StoreError>;

    /// Append a daily signal instance snapshot.
    #[allow(clippy::too_many_arguments)]
    async fn append_signal_instance_v2(
        &self,
        thread_id: i64,
        score: f64,
        impact: &str,
        trend: &str,
        article_count: i64,
        source_count: i64,
        avg_score: f64,
        entity_id: i64,
    ) -> Result<i64, StoreError>;

    /// Insert a signal timeline event.
    async fn insert_signal_event(
        &self,
        thread_id: i64,
        event_type: &str,
        payload: Option<&str>,
    ) -> Result<(), StoreError>;

    /// Load signal timeline events.
    async fn load_signal_events(&self, thread_id: i64, limit: u32) -> Result<Vec<SignalEvent>, StoreError>;

    /// Generate entity-anchored signal candidates with quality filters.
    async fn entity_signal_candidates_filtered(
        &self,
        now: i64,
        days: i64,
        limit: u32,
        min_entity_articles: u32,
        min_sources: u32,
    ) -> Result<Vec<EntitySignalCandidate>, StoreError>;
}
