use crate::types::{RankComponents, ScoredDecision, ScoredMemory, ScoredReflection};

/// RankingStrategy trait — pluggable ranking per category.
pub trait RankingStrategy<T> {
    fn score(&self, item: &T, components: &RankComponents) -> f64;
    fn rank(&self, items: Vec<T>) -> Vec<T>;
}

/// Default ranking formula used when no specific strategy exists.
pub fn default_score(components: &RankComponents) -> f64 {
    0.30 * components.query_alignment
        + 0.25 * components.confidence
        + 0.20 * components.recency
        + 0.15 * components.usage_frequency
        + 0.10 * components.user_specificity
}

pub struct DefaultRanking;
pub struct DecisionRanking;
pub struct MemoryRanking;
pub struct ReflectionRanking;

macro_rules! impl_ranking {
    ($name:ty) => {
        impl RankingStrategy<ScoredDecision> for $name {
            fn score(&self, _item: &ScoredDecision, components: &RankComponents) -> f64 {
                default_score(components)
            }
            fn rank(&self, mut items: Vec<ScoredDecision>) -> Vec<ScoredDecision> {
                items.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));
                items
            }
        }
        impl RankingStrategy<ScoredReflection> for $name {
            fn score(&self, _item: &ScoredReflection, components: &RankComponents) -> f64 {
                default_score(components)
            }
            fn rank(&self, mut items: Vec<ScoredReflection>) -> Vec<ScoredReflection> {
                items.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));
                items
            }
        }
        impl RankingStrategy<ScoredMemory> for $name {
            fn score(&self, _item: &ScoredMemory, components: &RankComponents) -> f64 {
                default_score(components)
            }
            fn rank(&self, mut items: Vec<ScoredMemory>) -> Vec<ScoredMemory> {
                items.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));
                items
            }
        }
    };
}

impl_ranking!(DefaultRanking);
impl_ranking!(DecisionRanking);
impl_ranking!(MemoryRanking);
impl_ranking!(ReflectionRanking);

pub fn apply_ranking_strategy<T, S: RankingStrategy<T>>(strategy: &S, items: Vec<T>) -> Vec<T> {
    strategy.rank(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_components(confidence: f64) -> RankComponents {
        RankComponents { query_alignment: 0.5, confidence, recency: 0.5, usage_frequency: 0.5, user_specificity: 1.0 }
    }

    #[test]
    fn default_score_higher_confidence_ranks_higher() {
        let low = default_score(&make_components(0.3));
        let high = default_score(&make_components(0.9));
        assert!(high > low);
    }

    #[test]
    fn decision_ranking_sorts_by_score() {
        let mut d1 = ScoredDecision::default();
        d1.relevance_score = 0.9;
        let mut d2 = ScoredDecision::default();
        d2.relevance_score = 0.5;
        let ranked = DecisionRanking.rank(vec![d2, d1]);
        assert_eq!(ranked.len(), 2);
        assert!((ranked[0].relevance_score - 0.9).abs() < 0.01);
    }
}
