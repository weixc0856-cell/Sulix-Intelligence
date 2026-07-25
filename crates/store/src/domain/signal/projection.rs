//! Signal projection — assemble domain data into projection views.
//!
//! NOTE: This module is superseded by `signal-engine/src/query/radar.rs`.
//! The `SignalQueryService::radar()` method produces the correct projection
//! with `Vec<RelatedEntityRef>` instead of `Vec<String>`. This file is
//! retained for reference but is no longer imported.
#![allow(dead_code)]

use crate::{RadarDashboardSummary, RadarResponse, SignalBriefInput, SignalHealth, SignalRadarItem};

/// Build a RadarResponse from active signal threads.
pub fn build_radar_response(
    inputs: Vec<SignalBriefInput>,
    health_scores: Vec<SignalHealth>,
    now: i64,
) -> RadarResponse {
    let mut signals: Vec<SignalRadarItem> = Vec::with_capacity(inputs.len());
    let mut rising = 0i64;
    let mut stable = 0i64;
    let mut decaying = 0i64;

    for (input, health) in inputs.into_iter().zip(health_scores) {
        match input.status.as_str() {
            "active" if health.score >= 0.5 => rising += 1,
            "active" => stable += 1,
            "decaying" => decaying += 1,
            _ => {}
        }

        let latest_inst = input.instances.first();
        let first_inst = input.instances.last();

        signals.push(SignalRadarItem {
            id: format!("thread_{}", input.thread_id),
            title: input.title,
            status: input.status,
            trend: input.trend,
            health,
            anchor_entity: None,
            evidence: crate::SignalEvidenceSummary {
                articles: input.recent_article_count,
                sources: input.source_count,
                avg_score: latest_inst.map(|i| i.score).unwrap_or(0.0),
                last_seen: latest_inst.map(|i| i.generated_at).unwrap_or(now),
                velocity_24h: (input.recent_article_count as f64 / 7.0).round() as i64,
            },
            related: Vec::new(),
            first_seen_at: first_inst.map(|i| i.generated_at).unwrap_or(now),
            last_evidence_at: latest_inst.map(|i| i.generated_at).unwrap_or(now),
        });
    }

    let total_active = rising + stable;

    RadarResponse {
        generated_at: now,
        summary: RadarDashboardSummary { total_active, rising, stable, decaying },
        signals,
    }
}
