//! Signal Thread Detail Query — thread + instances + events + evidence + analysis.
//!
//! This is the unified read model for the Signal Detail / Investigation page.
//! It merges three data sources:
//! 1. `intelligence_signals` instances → base timeline events
//! 2. `signal_events` table → stored timeline events (lifecycle, etc.)
//! 3. Entity relations → related entities with relation_type
//!
//! Also computes a rule-based "Why This Matters" analysis.
//!
//! Sprint 5.2+: Events are read from the R2 event archive via the SignalEventLog
//! port (event-store-backed in production),
//! with D1 signal_events as fallback.

use crate::error::SignalError;
use crate::models::{SignalAnalysis, SignalDetail, SignalTimelineEvent};
use crate::ports::{SignalEventLog, SignalQuery};

/// Build the full SignalDetail for a thread.
pub async fn build(
    query: &dyn SignalQuery,
    event_log: Option<&dyn SignalEventLog>,
    thread_id: i64,
) -> Result<Option<SignalDetail>, SignalError> {
    // 1. Load thread via the read-model port
    let detail = query.load_signal_detail(thread_id).await?;
    let mut detail = match detail {
        Some(d) => d,
        None => return Ok(None),
    };

    // 2. Merge stored events into timeline
    let merged = merge_signal_events(query, event_log, thread_id, &detail.timeline).await?;
    detail.timeline = merged;

    // 3. Build SignalAnalysis (structured, not appended to description)
    detail.analysis = build_analysis(&detail);

    Ok(Some(detail))
}

/// Merge stored signal events into the timeline.
///
/// Prefers the R2 event archive via [`SignalEventLog`], falling back to the D1
/// `signal_events` table for backward compatibility.
async fn merge_signal_events(
    query: &dyn SignalQuery,
    event_log: Option<&dyn SignalEventLog>,
    thread_id: i64,
    instance_timeline: &[SignalTimelineEvent],
) -> Result<Vec<SignalTimelineEvent>, SignalError> {
    // Try the event log first (R2 archive)
    if let Some(log) = event_log {
        let agg_id = format!("SIG-{thread_id:06}");
        match log.load(&agg_id, 50).await {
            Ok(events) if !events.is_empty() => {
                let mut merged: Vec<SignalTimelineEvent> = instance_timeline.to_vec();
                for e in events {
                    merged.push(SignalTimelineEvent {
                        timestamp: e.occurred_at,
                        event_type: e.event_type.clone(),
                        score: e.payload.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0),
                        article_count: e.payload.get("article_count").and_then(|v| v.as_i64()).unwrap_or(0),
                        description: e.event_type,
                    });
                }
                merged.sort_by_key(|b| std::cmp::Reverse(b.timestamp));
                merged.dedup_by(|a, b| a.timestamp == b.timestamp && a.event_type == b.event_type);
                return Ok(merged);
            }
            _ => {} // event log unavailable or empty — fall through to D1
        }
    }

    // D1 legacy fallback
    let events = query.load_signal_events(thread_id, 50).await?;

    if events.is_empty() {
        return Ok(instance_timeline.to_vec());
    }

    let mut merged: Vec<SignalTimelineEvent> = instance_timeline.to_vec();

    for e in events {
        let (score, article_count, description) = describe_event(&e.event_type, &e.payload);

        merged.push(SignalTimelineEvent {
            timestamp: e.created_at,
            event_type: e.event_type,
            score,
            article_count,
            description,
        });
    }

    // Sort by timestamp descending
    merged.sort_by_key(|b| std::cmp::Reverse(b.timestamp));
    merged.dedup_by(|a, b| a.timestamp == b.timestamp && a.event_type == b.event_type);
    Ok(merged)
}

/// Parse a signal event into structured timeline fields.
fn describe_event(event_type: &str, payload: &Option<String>) -> (f64, i64, String) {
    let parsed = payload.as_ref().and_then(|p| serde_json::from_str::<serde_json::Value>(p).ok());

    let score = parsed.as_ref().and_then(|v| v["score"].as_f64()).unwrap_or(0.0);
    let article_count = parsed.as_ref().and_then(|v| v["article_count"].as_i64()).unwrap_or(0);

    let description = match event_type {
        "created" => "Signal thread created".into(),
        "score_changed" => {
            if let Some(p) = parsed {
                let s = p["score"].as_f64().unwrap_or(0.0);
                let articles = p["article_count"].as_i64().unwrap_or(0);
                let trend = p["trend"].as_str().unwrap_or("stable");
                format!("Score changed to {:.1} ({articles} articles, trend: {trend})", s)
            } else {
                "Score changed".into()
            }
        }
        "status_changed" => "Status changed".into(),
        "accelerating" => "Signal accelerating — velocity increasing significantly".into(),
        "decaying" => "Signal decaying — activity decreasing".into(),
        "resolved" => "Signal resolved — no recent activity detected".into(),
        "evidence_added" => {
            let count = parsed.and_then(|p| p["count"].as_i64()).unwrap_or(0);
            format!("{} new evidence articles added", count)
        }
        _ => format!("Event: {}", event_type.replace('_', " ")),
    };

    (score, article_count, description)
}

/// Build a rule-based "Why This Matters" analysis.
///
/// Uses health components to generate structured insight without an LLM.
/// This is the bridge between raw signal metrics and Decision Loop input.
fn build_analysis(detail: &SignalDetail) -> Option<SignalAnalysis> {
    let h = &detail.health;
    let vol = h.components.volume;
    let qual = h.components.quality;
    let vel = h.components.velocity;

    // Confidence label
    let confidence_label = if h.score >= 0.6 {
        "High confidence"
    } else if h.score >= 0.3 {
        "Moderate confidence"
    } else {
        "Developing signal"
    };

    // Impact
    let impact = if h.score >= 0.6 {
        "High"
    } else if h.score >= 0.3 {
        "Moderate"
    } else {
        "Low"
    };

    // Velocity description
    let velocity_desc = if vel >= 0.7 {
        "rapidly increasing velocity"
    } else if vel >= 0.4 {
        "steady velocity"
    } else {
        "declining velocity"
    };

    // Why it matters
    let why_it_matters = format!(
        "{} — Signal shows {} activity across {} sources with {}. {} evidence articles, quality {:.0}%.",
        confidence_label,
        if vol >= 0.4 { "strong" } else { "developing" },
        detail.related_entities.len() + 1,
        velocity_desc,
        detail.evidence_top.len(),
        qual * 100.0,
    );

    // Confidence reason (data-driven)
    let confidence_reason = format!(
        "{} articles across {} source{} with {:.0}% quality score",
        detail.evidence_top.len(),
        detail.related_entities.len() + 1,
        if detail.related_entities.len() + 1 > 1 { "s" } else { "" },
        qual * 100.0,
    );

    Some(SignalAnalysis { why_it_matters, impact: impact.into(), confidence_reason })
}
