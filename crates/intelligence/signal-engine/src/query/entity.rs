//! Entity Signal Threads Query — threads anchored to an entity.
//!
//! Replaces the old `entity_signals()` which queried `intelligence_signals`
//! directly and returned empty titles. This query joins `signal_threads`
//! to return thread-level summaries with meaningful metadata.

use crate::error::SignalError;
use crate::models::SignalThreadFilter;
use crate::ports::SignalQuery;
use crate::query::SignalThreadSummary;

/// Load signal threads anchored to an entity.
///
/// Returns thread-level summaries (not individual instances),
/// because the product object is the SignalThread, not the instance.
pub async fn threads(
    query: &dyn SignalQuery,
    entity_id: i64,
    limit: u32,
) -> Result<Vec<SignalThreadSummary>, SignalError> {
    // Use the store's listing method filtered by anchor_entity_id — the signal
    // thread listing is the widest read across statuses, then this projection
    // narrows to the entity's own threads (matches by signal_key).

    let filter = SignalThreadFilter {
        statuses: vec!["active".into(), "decaying".into(), "resolved".into(), "archived".into()],
        limit,
        min_score: 0.0,
    };
    let threads = query.list_signal_threads(&filter).await?;

    let mut result: Vec<SignalThreadSummary> = Vec::new();

    for t in threads {
        // Filter by anchor entity if present
        if t.anchor_entity.as_deref() == Some("") && t.signal_key != format!("entity:{}", entity_id) {
            // Can't match by entity — skip ambiguous ones
            continue;
        }
        if !t.signal_key.starts_with("entity:") {
            continue;
        }
        // Extract entity_id from signal_key "entity:{id}"
        let key_entity_id = t.signal_key.strip_prefix("entity:").and_then(|s| s.parse::<i64>().ok());
        if key_entity_id != Some(entity_id) {
            continue;
        }

        let latest = t.instances.first();
        result.push(SignalThreadSummary {
            thread_id: t.thread_id,
            title: t.title,
            status: t.status,
            health_score: t.health_score,
            trend: t.trend,
            current_score: t.current_score,
            latest_impact: String::new(),
            instance_count: t.instances.len() as u32,
            total_articles: t.cumulative_article_count,
            first_seen_at: t.instances.last().map(|i| i.generated_at).unwrap_or(0),
            last_seen_at: latest.map(|i| i.generated_at).unwrap_or(0),
        });
    }

    Ok(result)
}
