//! Candidate filtering — quality gates for entity signal candidates.
//!
//! The primary filtering is done at the SQL level via
//! `entity_signal_candidates_filtered`; this module holds reference
//! implementations for testing.

#[cfg(test)]
use store::EntitySignalCandidate;

/// Minimum quality threshold for a candidate to become a signal thread.
///
/// Returns `true` if the candidate passes all quality gates:
/// - Entity type is known (not `"unknown"`)
/// - Has at least 2 distinct sources (multi-source evidence)
#[cfg(test)]
fn is_valid_candidate(candidate: &EntitySignalCandidate) -> bool {
    if candidate.entity_type.eq_ignore_ascii_case("unknown") {
        return false;
    }
    if candidate.source_count < 2 {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candidate(entity_type: &str, source_count: i64) -> EntitySignalCandidate {
        EntitySignalCandidate {
            entity_id: 1,
            entity_name: "Test".into(),
            entity_type: entity_type.into(),
            score: 0.5,
            volume: 0.0,
            diversity: 0.0,
            quality: 0.0,
            velocity: 0.0,
            novelty: 0.0,
            article_count: 3,
            source_count,
            avg_score: 0.5,
            trend: "stable".into(),
            evidence: vec![],
            related_entity_ids: vec![],
        }
    }

    #[test]
    fn rejects_unknown_type() {
        let c = make_candidate("unknown", 3);
        assert!(!is_valid_candidate(&c));
    }

    #[test]
    fn rejects_single_source() {
        let c = make_candidate("organization", 1);
        assert!(!is_valid_candidate(&c));
    }

    #[test]
    fn accepts_valid() {
        let c = make_candidate("organization", 3);
        assert!(is_valid_candidate(&c));
    }

    #[test]
    fn accepts_product() {
        let c = make_candidate("product", 5);
        assert!(is_valid_candidate(&c));
    }
}
