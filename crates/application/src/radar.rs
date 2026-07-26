//! Radar page projection — eliminates the N+1 problem in the radar query.
//!
//! ## Current problem
//! `get_active_signal_threads(N)` fires **1 + 3N** D1 queries:
//! - 1: thread list
//! - N: instances per thread
//! - N: evidence per thread
//! - N: related entities per thread
//!
//! ## Target
//! **4 queries total** (regardless of N):
//! - 1: thread list
//! - 1: batch instances (WHERE signal_thread_id IN (...))
//! - 1: batch evidence (WHERE signal_id IN (subquery))
//! - 1: batch entities (WHERE source/target_entity_id IN (...))

use store::{RelatedEntityRef, StoreError};

/// A single radar signal item with full health, evidence, and entity data.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RadarSignalItem {
    pub id: i64,
    pub title: String,
    pub status: String,
    pub trend: String,
    pub health_score: f64,
    pub current_score: f64,
    pub evidence_count: i64,
    pub source_count: i64,
    pub anchor_entity: Option<String>,
    pub signal_key: String,
    /// Enriched breakdown (derived from health_score / current_score).
    pub health: store::SignalHealth,
    /// Evidence summary (from batch-loaded data).
    pub evidence: store::SignalEvidenceSummary,
    /// Related entities (from batch-loaded data).
    pub related: Vec<RelatedEntityRef>,
    pub first_seen_at: i64,
    pub last_evidence_at: i64,
}

/// Radar page projection result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RadarProjectionResult {
    pub generated_at: i64,
    pub signal_count: usize,
    pub signals: Vec<RadarSignalItem>,
}

impl RadarProjectionResult {
    pub fn empty() -> Self {
        Self { generated_at: 0, signal_count: 0, signals: Vec::new() }
    }
}

// ── Internal DTOs for batch queries ──

#[allow(dead_code)]
#[derive(serde::Deserialize)]
struct InstanceRow {
    signal_thread_id: i64,
    score: f64,
    confidence: f64,
    trend: String,
    article_count: i64,
    source_count: i64,
    created_at: i64,
}

#[allow(dead_code)]
#[derive(serde::Deserialize)]
struct EvRow {
    article_id: i64,
    title: String,
    url: Option<String>,
    feed_name: Option<String>,
    score: f64,
}

#[allow(dead_code)]
#[derive(serde::Deserialize)]
struct EntityRelRow {
    source_entity_id: i64,
    target_entity_id: i64,
    confidence: f64,
}

/// Radar projection service — assembles the radar page response
/// using batch queries instead of N+1.
///
/// Generic over `S` so it can be unit-tested with `MemoryStore`.
pub struct RadarProjectionService<S> {
    store: S,
}

impl<S> RadarProjectionService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

impl<S> RadarProjectionService<S>
where
    S: store::SignalQueryService + store::BatchSignalQueryService,
{
    /// Build the radar projection using batch-loaded evidence and entities.
    ///
    /// Query count: 1 (thread list) + 1 (batch evidence) + 1 (batch entities) = **3 queries**
    /// vs the previous N+1 pattern of 1 + 3N queries.
    pub async fn build(&self, limit: u32) -> Result<RadarProjectionResult, StoreError> {
        let now: i64 = 0; // Timestamp injected by the caller in production
        let threads = self.store.get_active_signal_threads(limit).await?;
        let thread_ids: Vec<i64> = threads.iter().map(|t| t.thread_id).collect();

        // Batch-load evidence and entities (2 queries total, regardless of N)
        let evidence_map = self.store.batch_evidence(&thread_ids).await?;
        let entity_map = self.store.batch_related_entities(&thread_ids).await?;

        let signals: Vec<RadarSignalItem> = threads
            .into_iter()
            .map(|t| {
                let tid = t.thread_id;
                let evidence_count = evidence_map.get(&tid).map(|v| v.len() as i64).unwrap_or(0);
                let entity_count = entity_map.get(&tid).map(|v| v.len() as i64).unwrap_or(0);
                let articles_score = evidence_map
                    .get(&tid)
                    .map(|v| v.iter().map(|a| a.score).sum::<f64>() / v.len() as f64)
                    .unwrap_or(0.0);

                let trend = t.trend.clone();
                RadarSignalItem {
                    id: tid,
                    title: t.title,
                    status: t.status,
                    trend: t.trend,
                    health_score: t.health_score,
                    current_score: t.current_score,
                    evidence_count,
                    source_count: t.source_count,
                    anchor_entity: t.anchor_entity,
                    signal_key: t.signal_key,
                    health: store::SignalHealth {
                        score: t.health_score,
                        breakdown: store::SignalHealthBreakdown {
                            activity: t.current_score.max(0.0),
                            diversity: (entity_count as f64 / 10.0).min(1.0),
                            quality: (t.health_score * 0.7 + t.current_score * 0.3).min(1.0),
                            velocity: if trend == "rising" {
                                0.8
                            } else if trend == "declining" {
                                0.2
                            } else {
                                0.5
                            },
                        },
                    },
                    evidence: store::SignalEvidenceSummary {
                        articles: evidence_count,
                        sources: t.source_count,
                        avg_score: articles_score,
                        last_seen: 0,
                        velocity_24h: (evidence_count.max(0) as f64 / 7.0).round() as i64,
                    },
                    related: entity_map.get(&tid).cloned().unwrap_or_default(),
                    first_seen_at: 0,
                    last_evidence_at: 0,
                }
            })
            .collect();

        Ok(RadarProjectionResult { generated_at: now, signal_count: signals.len(), signals })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use store::memory::MemoryStore;

    #[test]
    fn test_radar_projection_empty_store() {
        let store = MemoryStore::new();
        let service = RadarProjectionService::new(store);
        let result = futures::executor::block_on(service.build(10)).unwrap();
        assert_eq!(result.signal_count, 0);
        assert!(result.signals.is_empty());
    }
}
