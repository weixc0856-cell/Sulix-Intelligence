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

use store::{StoreBackend, StoreError};

use crate::ports::SignalEventLog;

/// Unified query service for Intelligence read models.
pub struct SignalQueryService<'a, S: StoreBackend> {
    pub store: &'a S,
    pub event_log: Option<&'a dyn SignalEventLog>,
}

impl<'a, S: StoreBackend> SignalQueryService<'a, S> {
    pub fn new(store: &'a S) -> Self {
        Self { store, event_log: None }
    }

    /// Thread detail — thread + instances + signal_events + evidence + entities.
    pub async fn thread_detail(&self, thread_id: i64) -> Result<Option<store::SignalDetail>, StoreError> {
        detail::build(self.store, self.event_log, thread_id).await
    }

    /// Entity signal threads — threads anchored to an entity.
    pub async fn entity_threads(&self, entity_id: i64, limit: u32) -> Result<Vec<SignalThreadSummary>, StoreError> {
        entity::threads(self.store, entity_id, limit).await
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
