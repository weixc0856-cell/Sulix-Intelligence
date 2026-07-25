//! Signal Merge Pipeline — combines entity-driven and semantic discovery
//! signals into deduplicated signal threads.
//!
//! Uses article-level overlap (extracted from evidence) rather than instance IDs
//! so that two signals referencing the same articles can be correctly merged.

use store::SignalBriefInput;

/// Merge two sets of signal inputs, deduplicating by article overlap.
///
/// When two signals share >50% of their articles, they are merged into
/// a single thread with `discovery_method = 'hybrid'`.
pub fn merge_signals(
    entity_signals: Vec<SignalBriefInput>,
    semantic_signals: Vec<SignalBriefInput>,
) -> Vec<SignalBriefInput> {
    let mut merged: Vec<SignalBriefInput> = entity_signals;

    for semantic in semantic_signals {
        let mut should_insert = true;
        let semantic_articles = extract_article_ids(&semantic);
        for existing in &mut merged {
            let existing_articles = extract_article_ids(existing);
            let overlap = count_article_overlap(&semantic_articles, &existing_articles);
            let total = semantic_articles.len().max(existing_articles.len());
            if total > 0 && overlap as f64 / total as f64 > 0.5 {
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

/// Extract article IDs from a signal's evidence list.
fn extract_article_ids(input: &SignalBriefInput) -> Vec<i64> {
    input.evidence.iter().map(|a| a.id).collect()
}

/// Count overlapping articles between two sets of article IDs.
fn count_article_overlap(a: &[i64], b: &[i64]) -> usize {
    let set_a: std::collections::HashSet<i64> = a.iter().copied().collect();
    let set_b: std::collections::HashSet<i64> = b.iter().copied().collect();
    set_a.intersection(&set_b).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use store::{BriefArticle, SignalProvenance};

    fn make_test_signal(id: i64, article_ids: &[i64]) -> SignalBriefInput {
        SignalBriefInput {
            thread_id: id,
            signal_key: format!("test:{}", id),
            anchor_entity: None,
            title: format!("Signal {}", id),
            description: String::new(),
            status: "active".into(),
            health_score: 0.5,
            current_score: 0.5,
            trend: "stable".into(),
            cumulative_article_count: article_ids.len() as i64,
            recent_article_count: article_ids.len() as i64,
            source_count: 1,
            velocity: 0.5,
            instances: vec![],
            evidence: article_ids
                .iter()
                .map(|&aid| BriefArticle {
                    id: aid,
                    title: format!("Article {}", aid),
                    url: None,
                    feed_name: None,
                    score: 0.5,
                })
                .collect(),
            related_entities: vec![],
            provenance: SignalProvenance::default(),
        }
    }

    #[test]
    fn test_merge_overlapping_signals() {
        // Signal A has articles [1,2,3], Signal B has articles [2,3,4]
        // Overlap = {2,3} → 2/3 = 67% > 50% → should merge
        let entity = vec![make_test_signal(1, &[1, 2, 3])];
        let semantic = vec![make_test_signal(2, &[2, 3, 4])];
        let merged = merge_signals(entity, semantic);
        assert_eq!(merged.len(), 1, "should merge overlapping signals");
    }

    #[test]
    fn test_no_overlap_keeps_separate() {
        // Signal A has articles [1,2], Signal B has articles [3,4]
        // Overlap = {} → should stay separate
        let entity = vec![make_test_signal(1, &[1, 2])];
        let semantic = vec![make_test_signal(2, &[3, 4])];
        let merged = merge_signals(entity, semantic);
        assert_eq!(merged.len(), 2, "should keep non-overlapping signals");
    }

    #[test]
    fn test_empty_signals() {
        let merged = merge_signals(vec![], vec![]);
        assert!(merged.is_empty());
    }
}
