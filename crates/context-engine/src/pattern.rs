use crate::types::{PatternContext, ScoredDecision, ScoredReflection};

/// PatternDetector trait — pluggable for future pattern-engine crate.
pub trait PatternDetector {
    fn detect(&self, decisions: &[ScoredDecision], _reflections: &[ScoredReflection]) -> Vec<PatternContext>;
}

/// Default implementation: groups by decision_type, flags repeated failures.
pub struct DefaultPatternDetector;

impl PatternDetector for DefaultPatternDetector {
    fn detect(&self, decisions: &[ScoredDecision], _reflections: &[ScoredReflection]) -> Vec<PatternContext> {
        let mut type_counts: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
        for d in decisions {
            *type_counts.entry(d.decision_type.as_str()).or_insert(0) += 1;
        }
        type_counts
            .into_iter()
            .filter(|(_, count)| *count >= 2)
            .map(|(dtype, count)| PatternContext {
                pattern_type: "recurring_theme".into(),
                description: format!("You have made {} decisions in '{}'", count, dtype),
                frequency: count,
                evidence_refs: decisions.iter().filter(|d| d.decision_type == dtype).map(|d| d.id.clone()).collect(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{RankComponents, ScoredDecision};

    fn make_decision(id: &str, dtype: &str) -> ScoredDecision {
        ScoredDecision {
            id: id.into(),
            title: "".into(),
            decision_type: dtype.into(),
            status: "active".into(),
            confidence: 0.5,
            relevance_score: 0.0,
            rank_components: RankComponents {
                query_alignment: 0.0,
                confidence: 0.5,
                recency: 0.0,
                usage_frequency: 0.0,
                user_specificity: 1.0,
            },
        }
    }

    #[test]
    fn repeated_type_detects_pattern() {
        let decisions = vec![make_decision("DEC-001", "investment"), make_decision("DEC-002", "investment")];
        let detector = DefaultPatternDetector;
        let patterns = detector.detect(&decisions, &[]);
        assert_eq!(patterns.len(), 1);
        assert!(patterns[0].description.contains("investment"));
        assert_eq!(patterns[0].frequency, 2);
    }

    #[test]
    fn single_decision_no_pattern() {
        let decisions = vec![make_decision("DEC-001", "career")];
        let detector = DefaultPatternDetector;
        let patterns = detector.detect(&decisions, &[]);
        assert!(patterns.is_empty());
    }
}
