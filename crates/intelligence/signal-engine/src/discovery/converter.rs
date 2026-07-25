//! Converter — transforms semantic discovery results into SignalCandidate.

use crate::discovery::clustering::{ArticleCluster, SimilarityEdge};
use crate::scoring::semantic::{signal_score_v2, SemanticClusterScoreInput};
use crate::source::SignalCandidate;

/// Convert a cluster + edges into a signal candidate.
#[allow(dead_code)]
pub fn cluster_to_candidate(cluster: &ArticleCluster, edges: &[SimilarityEdge]) -> SignalCandidate {
    let article_ids = &cluster.article_ids;
    // Compute cluster-level edges for cohesion
    let cluster_edges: Vec<(i64, i64, f64)> = edges
        .iter()
        .filter(|e| article_ids.contains(&e.article_a) && article_ids.contains(&e.article_b))
        .map(|e| (e.article_a, e.article_b, e.similarity))
        .collect();

    let avg_sim = crate::discovery::similarity::avg_pairwise_similarity(&cluster_edges);

    let score_input = SemanticClusterScoreInput {
        avg_similarity: avg_sim,
        entity_focus: 0.5,
        temporal_momentum: 0.5,
        source_diversity: 0.5,
    };
    let v2_score = signal_score_v2(&score_input);

    SignalCandidate {
        signal_key: crate::discovery::similarity::cluster_id(article_ids),
        anchor_entity_id: None,
        title: format!("cluster:{}", article_ids.first().copied().unwrap_or(0)),
        status: "active".into(),
        discovery_method: store::DiscoveryMethod::Semantic,
        discovery_score: Some(v2_score),
        score: v2_score,
        trend: "rising".into(),
        article_count: article_ids.len() as i64,
        source_count: 0,
        avg_score: avg_sim,
        evidence: vec![],
        related_entities: vec![],
    }
}
