//! Radar Query — build radar projection from active signal threads.
//!
//! Replaces the old `store::domain::signal::projection::build_radar_response`
//! with a version that uses the unified Read Model:
//! - Includes `RelatedEntityRef` with `relation_type` instead of bare `string[]`
//! - Includes `anchor_entity` from the thread data
//! - Health is computed by `calculate_signal_health`

use store::{SignalHealth, SignalHealthBreakdown, StoreBackend, StoreError};

use crate::query::{RadarProjection, RadarSignal, RadarSummary};
use crate::scoring::health::{calculate_health, health_breakdown, SignalHealthMetrics};

/// Build the radar projection from active signal threads.
pub async fn build(store: &impl StoreBackend, now: i64) -> Result<RadarProjection, StoreError> {
    let threads = store.get_active_signal_threads(50).await?;

    let mut signals: Vec<RadarSignal> = Vec::with_capacity(threads.len());
    let mut rising = 0i64;
    let mut stable = 0i64;
    let mut decaying = 0i64;

    for input in &threads {
        // Compute health
        let days_active = if input.instances.len() > 1 {
            let first = input.instances.last().map(|i| i.generated_at).unwrap_or(now);
            let last = input.instances.first().map(|i| i.generated_at).unwrap_or(now);
            ((last - first) as f64 / 86400.0).max(1.0)
        } else {
            1.0
        };
        let metrics = SignalHealthMetrics {
            article_count: input.recent_article_count,
            source_count: input.source_count,
            avg_score: input.instances.first().map(|i| i.score).unwrap_or(0.0),
            trend: input.trend.clone(),
            days_active,
        };
        let score = calculate_health(&metrics);
        let (activity, diversity, quality, velocity) = health_breakdown(&metrics);
        let health =
            SignalHealth { score, breakdown: SignalHealthBreakdown { activity, diversity, quality, velocity } };

        // Count summary buckets
        match input.status.as_str() {
            "active" if health.score >= 0.5 => rising += 1,
            "active" => stable += 1,
            "decaying" => decaying += 1,
            _ => {}
        }

        // Load related entities with relation_type
        let related = store.load_thread_related_entities(input.thread_id, 5).await?;

        let latest_inst = input.instances.first();
        let first_inst = input.instances.last();

        signals.push(RadarSignal {
            id: format!("thread_{}", input.thread_id),
            title: input.title.clone(),
            status: input.status.clone(),
            trend: input.trend.clone(),
            health,
            anchor_entity: input.anchor_entity.as_deref().map(|name| store::EntitySignalRef {
                id: 0,
                name: name.to_string(),
                entity_type: String::new(),
            }),
            evidence: store::SignalEvidenceSummary {
                articles: input.recent_article_count,
                sources: input.source_count,
                avg_score: latest_inst.map(|i| i.score).unwrap_or(0.0),
                last_seen: latest_inst.map(|i| i.generated_at).unwrap_or(now),
                velocity_24h: (input.recent_article_count as f64 / 7.0).round() as i64,
            },
            related,
            first_seen_at: first_inst.map(|i| i.generated_at).unwrap_or(now),
            last_evidence_at: latest_inst.map(|i| i.generated_at).unwrap_or(now),
        });
    }

    // Sort by health score descending
    signals.sort_by(|a, b| b.health.score.partial_cmp(&a.health.score).unwrap_or(std::cmp::Ordering::Equal));

    let total_active = rising + stable;

    Ok(RadarProjection { generated_at: now, summary: RadarSummary { total_active, rising, stable, decaying }, signals })
}
