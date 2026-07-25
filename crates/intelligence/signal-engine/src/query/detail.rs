//! Signal Thread Detail Query — thread + instances + events + evidence + summary.
//!
//! This is the unified read model for the Signal Detail / Investigation page.
//! It merges three data sources:
//! 1. `intelligence_signals` instances → base timeline events
//! 2. `signal_events` table → stored timeline events (lifecycle, etc.)
//! 3. Entity relations → related entities with relation_type
//!
//! Also computes a rule-based "Why This Matters" summary.

use store::{SignalDetail, SignalTimelineEvent, StoreBackend, StoreError};

/// Build the full SignalDetail for a thread.
pub async fn build(store: &impl StoreBackend, thread_id: i64) -> Result<Option<SignalDetail>, StoreError> {
    // 1. Load thread via existing detail method
    let detail = store.load_signal_detail(thread_id).await?;
    let mut detail = match detail {
        Some(d) => d,
        None => return Ok(None),
    };

    // 2. Merge signal_events into timeline
    let merged = merge_signal_events(store, thread_id, &detail.timeline).await?;
    detail.timeline = merged;

    // 3. Add "Why This Matters" summary
    let summary = build_signal_summary(&detail);
    // For now we store summary as a serialised field since SignalDetail
    // already uses `description` for this purpose.
    // The frontend renders `description` below the title.
    if !detail.description.is_empty() {
        // Keep existing description but append summary
        detail.description = format!("{}\n\n---\n{}", detail.description, summary);
    } else {
        detail.description = summary;
    }

    Ok(Some(detail))
}

/// Merge stored signal_events into the timeline alongside instance-based events.
async fn merge_signal_events(
    store: &impl StoreBackend,
    thread_id: i64,
    instance_timeline: &[SignalTimelineEvent],
) -> Result<Vec<SignalTimelineEvent>, StoreError> {
    let events = store.load_signal_events(thread_id, 50).await?;

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

/// Build a rule-based "Why This Matters" summary for the signal thread.
fn build_signal_summary(detail: &SignalDetail) -> String {
    let confidence = if detail.health.score >= 0.6 {
        "High confidence"
    } else if detail.health.score >= 0.3 {
        "Moderate confidence"
    } else {
        "Developing signal"
    };

    let vol = detail.health.components.volume;
    let divers = detail.health.components.diversity;
    let qual = detail.health.components.quality;
    let vel = detail.health.components.velocity;

    let volume_note = if vol >= 0.7 {
        "strong volume"
    } else if vol >= 0.4 {
        "moderate volume"
    } else {
        "low volume"
    };

    let diversity_note = if divers >= 0.5 { "across multiple independent sources" } else { "from limited sources" };

    let velocity_note = if vel >= 0.7 {
        "with rapidly increasing velocity"
    } else if vel >= 0.4 {
        "with steady velocity"
    } else {
        "with declining velocity"
    };

    let entities_note = if !detail.related_entities.is_empty() {
        let names: Vec<&str> = detail.related_entities.iter().map(|e| e.name.as_str()).take(3).collect();
        format!("Related entities: {}", names.join(", "))
    } else {
        String::new()
    };

    let mut summary = format!(
        "{} — Signal shows {} {} {}. {} evidence articles, quality {:.0}%.",
        confidence,
        volume_note,
        diversity_note,
        velocity_note,
        detail.evidence_top.len(),
        qual * 100.0,
    );

    if !entities_note.is_empty() {
        summary.push_str(&format!("\n{}", entities_note));
    }

    summary
}
