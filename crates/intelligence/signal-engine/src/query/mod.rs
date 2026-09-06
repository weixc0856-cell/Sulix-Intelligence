//! Signal Query Service — unified read model for Intelligence UI.
//!
//! All Intelligence API endpoints should call `SignalQueryService` methods
//! instead of directly querying store tables. This prevents the "write
//! model ≠ read model" drift that occurs when multiple consumers each
//! write their own interpretation of the data.
//!
//! ## Why a read model layer?
//!
//! Before this module existed, the Radar, Signal Detail, and Entity pages
//! each queried `signal_threads` / `intelligence_signals` / `signal_events`
//! independently, with different field interpretations and inconsistent
//! aggregation logic. The Query Service is the single source of truth for
//! "what Intelligence data looks like on screen."

pub mod detail;
pub mod entity;

use crate::error::SignalError;
use crate::models::SignalDetail;
use crate::ports::{SignalEventLog, SignalQuery};

/// Unified query service for Intelligence read models.
///
/// Reads through the [`SignalQuery`] boundary (store-backed adapter in
/// infrastructure) so the read-model assembly never depends on `store`
/// directly.
pub struct SignalQueryService<'a> {
    pub query: &'a dyn SignalQuery,
    pub event_log: Option<&'a dyn SignalEventLog>,
}

impl<'a> SignalQueryService<'a> {
    pub fn new(query: &'a dyn SignalQuery) -> Self {
        Self { query, event_log: None }
    }

    /// Attach the event log so `thread_detail` merges stored events from the
    /// R2 archive (falling back to the D1 `signal_events` table when the log is
    /// absent or empty). Without it, the timeline silently skips the stored
    /// events the engine dual-writes — the read/write divergence fixed 2026-09-06.
    pub fn with_event_log(mut self, event_log: &'a dyn SignalEventLog) -> Self {
        self.event_log = Some(event_log);
        self
    }

    /// Thread detail — thread + instances + signal_events + evidence + entities.
    pub async fn thread_detail(&self, thread_id: i64) -> Result<Option<SignalDetail>, SignalError> {
        detail::build(self.query, self.event_log, thread_id).await
    }

    /// Entity signal threads — threads anchored to an entity.
    pub async fn entity_threads(&self, entity_id: i64, limit: u32) -> Result<Vec<SignalThreadSummary>, SignalError> {
        entity::threads(self.query, entity_id, limit).await
    }
}

/// Lightweight summary of a signal thread for entity profile / listing views.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SignalThreadSummary {
    pub thread_id: i64,
    pub title: String,
    pub status: String,
    pub health_score: f64,
    pub trend: String,
    pub current_score: f64,
    pub latest_impact: String,
    pub instance_count: u32,
    pub total_articles: i64,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
}

#[cfg(test)]
mod tests {
    //! Regression: the query service *always* left the event log unset, so detail
    //! timelines never surfaced the stored events the engine dual-writes (read
    //! side silently fell back to the legacy D1 `signal_events` table the engine
    //! no longer writes). `with_event_log` + these tests pin the fix (2026-09-06).

    use super::*;
    use crate::models::{
        HealthComponents, SignalDetail, SignalEventRecord, SignalHealthDetail2, SignalThreadFilter, SignalThreadRow,
    };
    use crate::ports::SignalEvent;
    use async_trait::async_trait;

    fn stub_detail() -> SignalDetail {
        SignalDetail {
            id: 1,
            title: "CUDA".into(),
            description: String::new(),
            status: "active".into(),
            trend: "rising".into(),
            health: SignalHealthDetail2 {
                score: 0.5,
                components: HealthComponents {
                    volume: 0.5,
                    diversity: 0.5,
                    quality: 0.5,
                    velocity: 0.5,
                    persistence: 0.5,
                },
            },
            anchor_entity: None,
            first_seen_at: 100,
            last_seen_at: 200,
            timeline: Vec::new(),
            evidence_top: Vec::new(),
            related_entities: Vec::new(),
            related_signals: Vec::new(),
            analysis: None,
        }
    }

    struct StubQuery;

    #[async_trait(?Send)]
    impl SignalQuery for StubQuery {
        async fn load_signal_detail(&self, _thread_id: i64) -> Result<Option<SignalDetail>, crate::SignalError> {
            Ok(Some(stub_detail()))
        }
        async fn load_signal_events(
            &self,
            _thread_id: i64,
            _limit: u32,
        ) -> Result<Vec<SignalEventRecord>, crate::SignalError> {
            Ok(Vec::new())
        }
        async fn list_signal_threads(
            &self,
            _filter: &SignalThreadFilter,
        ) -> Result<Vec<SignalThreadRow>, crate::SignalError> {
            Ok(Vec::new())
        }
    }

    struct StubEventLog;

    #[async_trait(?Send)]
    impl SignalEventLog for StubEventLog {
        async fn append(&self, _event: &SignalEvent, _sequence: u64) -> Result<(), crate::SignalError> {
            Ok(())
        }
        async fn load(&self, _aggregate_id: &str, _limit: u32) -> Result<Vec<SignalEvent>, crate::SignalError> {
            Ok(vec![SignalEvent {
                event_type: "SignalScoreChanged".into(),
                aggregate_id: "SIG-000001".into(),
                payload: serde_json::json!({ "score": 0.8, "article_count": 3 }),
                occurred_at: 300,
            }])
        }
    }

    #[test]
    fn thread_detail_merges_r2_events_when_event_log_attached() {
        let query = StubQuery;
        let log = StubEventLog;
        let detail = futures::executor::block_on(SignalQueryService::new(&query).with_event_log(&log).thread_detail(1))
            .unwrap()
            .unwrap();
        assert_eq!(detail.timeline.len(), 1);
        assert_eq!(detail.timeline[0].event_type, "SignalScoreChanged");
        assert_eq!(detail.timeline[0].timestamp, 300);
    }

    #[test]
    fn thread_detail_without_event_log_keeps_stored_timeline_empty() {
        let query = StubQuery;
        let detail = futures::executor::block_on(SignalQueryService::new(&query).thread_detail(1)).unwrap().unwrap();
        assert!(detail.timeline.is_empty());
    }
}
