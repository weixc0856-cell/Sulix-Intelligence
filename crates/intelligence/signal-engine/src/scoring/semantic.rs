//! V2 Semantic Signal Scoring — the four-factor formula.
//!
//! Signal Score = Semantic Cohesion(0.3) × Entity Focus(0.25)
//!              × Temporal Momentum(0.25) × Source Diversity(0.2)

/// A semantic cluster ready for V2 scoring.
#[derive(Debug, Clone)]
pub struct SemanticClusterScoreInput {
    /// Average pairwise cosine similarity within the cluster (0.0–1.0).
    pub avg_similarity: f64,
    /// Max entity mention count ÷ total entity mentions in cluster.
    pub entity_focus: f64,
    /// Recent article rate ÷ historical article rate (velocity ratio).
    pub temporal_momentum: f64,
    /// Unique source count ÷ total articles.
    pub source_diversity: f64,
}

/// Compute the V2 signal score using the four-factor formula.
///
/// Returns a value in [0.0, 1.0] where higher = stronger signal.
pub fn signal_score_v2(input: &SemanticClusterScoreInput) -> f64 {
    let cohesion = input.avg_similarity.clamp(0.0, 1.0);
    let focus = input.entity_focus.clamp(0.0, 1.0);
    let momentum = input.temporal_momentum.clamp(0.0, 1.0);
    let diversity = input.source_diversity.clamp(0.0, 1.0);

    0.30 * cohesion + 0.25 * focus + 0.25 * momentum.min(1.0) + 0.20 * diversity
}

/// Compute entity focus for a set of articles.
///
/// Entity focus = max(entity_mention_count) / total_entity_mentions.
/// High focus means the cluster is centered on one dominant entity.
pub fn entity_focus(entity_mention_counts: &[u64]) -> f64 {
    let total: u64 = entity_mention_counts.iter().sum();
    if total == 0 {
        return 0.0;
    }
    let max = entity_mention_counts.iter().max().copied().unwrap_or(0);
    max as f64 / total as f64
}

/// Compute temporal momentum as the ratio of recent (3d) to historical (3-6d) article rates.
///
/// Returns 0.0–1.0+ where >1.0 means accelerating, <1.0 means decaying.
pub fn temporal_momentum(recent_count: f64, historical_count: f64) -> f64 {
    if historical_count <= 0.0 {
        return 1.0; // new cluster, full momentum
    }
    recent_count / historical_count
}

/// Compute source diversity as unique sources / total articles.
///
/// Higher = more corroboration from independent sources.
pub fn source_diversity(unique_sources: usize, total_articles: usize) -> f64 {
    if total_articles == 0 {
        return 0.0;
    }
    (unique_sources as f64 / total_articles as f64).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_v2_score_formula() {
        let input = SemanticClusterScoreInput {
            avg_similarity: 0.9,
            entity_focus: 0.8,
            temporal_momentum: 0.7,
            source_diversity: 0.6,
        };
        let score = signal_score_v2(&input);
        // 0.30 * 0.9 + 0.25 * 0.8 + 0.25 * 0.7 + 0.20 * 0.6
        // = 0.27 + 0.20 + 0.175 + 0.12 = 0.765
        assert!((score - 0.765).abs() < 0.01);
    }

    #[test]
    fn test_v2_score_clamps() {
        let input = SemanticClusterScoreInput {
            avg_similarity: 1.5,    // out of range
            entity_focus: -0.5,     // out of range
            temporal_momentum: 2.0, // capped
            source_diversity: 0.5,
        };
        let score = signal_score_v2(&input);
        assert!((0.0..=1.0).contains(&score));
    }

    #[test]
    fn test_entity_focus_single_dominant() {
        // 50 NVIDIA mentions out of 80 total → 0.625
        let counts = vec![50, 10, 10, 10];
        assert!((entity_focus(&counts) - 0.625).abs() < 0.01);
    }

    #[test]
    fn test_entity_focus_uniform() {
        let counts = vec![10, 10, 10];
        assert!((entity_focus(&counts) - 0.333).abs() < 0.01);
    }

    #[test]
    fn test_entity_focus_empty() {
        assert_eq!(entity_focus(&[]), 0.0);
    }

    #[test]
    fn test_temporal_momentum_accelerating() {
        let m = temporal_momentum(30.0, 10.0);
        assert!(m > 1.0);
    }

    #[test]
    fn test_temporal_momentum_decaying() {
        let m = temporal_momentum(5.0, 20.0);
        assert!(m < 1.0);
    }

    #[test]
    fn test_temporal_momentum_new() {
        let m = temporal_momentum(10.0, 0.0);
        assert_eq!(m, 1.0);
    }

    #[test]
    fn test_source_diversity() {
        assert!((source_diversity(5, 10) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_source_diversity_empty() {
        assert_eq!(source_diversity(0, 0), 0.0);
    }
}
