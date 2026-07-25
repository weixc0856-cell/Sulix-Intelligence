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
pub mod radar;

use event_store::EventStore;
use store::{RelatedEntityRef, StoreBackend, StoreError};

/// Unified query service for Intelligence read models.
pub struct SignalQueryService<'a, S: StoreBackend> {
    pub store: &'a S,
    pub event_store: Option<&'a dyn EventStore>,
}

impl<'a, S: StoreBackend> SignalQueryService<'a, S> {
    pub fn new(store: &'a S) -> Self {
        Self { store, event_store: None }
    }

    pub fn with_event_store(store: &'a S, event_store: &'a dyn EventStore) -> Self {
        Self { store, event_store: Some(event_store) }
    }

    /// Radar dashboard — active threads with health projection.
    pub async fn radar(&self, now: i64) -> Result<RadarProjection, StoreError> {
        radar::build(self.store, now).await
    }

    /// Thread detail — thread + instances + signal_events + evidence + entities.
    pub async fn thread_detail(&self, thread_id: i64) -> Result<Option<store::SignalDetail>, StoreError> {
        detail::build(self.store, self.event_store, thread_id).await
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

/// Radar projection — active signal threads with health scores.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RadarProjection {
    pub generated_at: i64,
    pub summary: RadarSummary,
    pub signals: Vec<RadarSignal>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RadarSummary {
    pub total_active: i64,
    pub rising: i64,
    pub stable: i64,
    pub decaying: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RadarSignal {
    pub id: String,
    pub title: String,
    pub status: String,
    pub trend: String,
    pub health: store::SignalHealth,
    pub anchor_entity: Option<store::EntitySignalRef>,
    pub evidence: store::SignalEvidenceSummary,
    pub related: Vec<RelatedEntityRef>,
    pub first_seen_at: i64,
    pub last_evidence_at: i64,
}
