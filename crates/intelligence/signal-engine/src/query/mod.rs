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
