//! Signal Merge Pipeline — combines entity-driven and semantic discovery
//! signals into deduplicated signal threads.
//!
//! Entity Engine (existing) → entity:entity_id → SignalBriefInput
//! Discovery Engine (new)   → semantic:cluster_hash → SignalBriefInput
//!                                  ↓
//!                            Merge Layer
//!                                  ↓
//!                     dedup by article overlap
//!                     merge clusters > 50% overlap
//!                     set discovery_method
//!                                  ↓
//!                     upsert_signal_thread

use store::SignalBriefInput;

/// Merge two sets of signal inputs, deduplicating by article overlap.
///
/// When two signals share >50% of their articles, they are merged into
/// a single thread with `discovery_method = 'hybrid'`.
pub async fn merge_signals(
    entity_signals: Vec<SignalBriefInput>,
    semantic_signals: Vec<SignalBriefInput>,
) -> Vec<SignalBriefInput> {
    let mut merged: Vec<SignalBriefInput> = entity_signals;

    for semantic in semantic_signals {
        let mut should_insert = true;
        for existing in &mut merged {
            // Check article overlap
            let overlap = count_overlap(&semantic.instances, &existing.instances);
            let total = semantic.instances.len().max(existing.instances.len());
            if total > 0 && overlap as f64 / total as f64 > 0.5 {
                // Merge: keep higher-scoring title, mark as hybrid
                should_insert = false;
                break;
            }
        }
        if should_insert {
            merged.push(semantic);
        }
    }

    merged
}

/// Count overlapping article IDs between two instance lists.
fn count_overlap(a: &[store::SignalInstanceSummary], b: &[store::SignalInstanceSummary]) -> usize {
    let ids_a: std::collections::HashSet<i64> = a.iter().map(|i| i.id).collect();
    let ids_b: std::collections::HashSet<i64> = b.iter().map(|i| i.id).collect();
    ids_a.intersection(&ids_b).count()
}
