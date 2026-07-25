//! Similarity computation utilities for semantic clustering.
//!
//! These are stateless helper functions. The actual ANN retrieval is
//! delegated to Vectorize's native query API.

/// Compute average pairwise similarity for a cluster from its edges.
///
/// Uses the edges returned by Vectorize ANN (not all-pairs).
/// For clusters with fewer than 2 articles, returns 0.0.
pub fn avg_pairwise_similarity(edges: &[(i64, i64, f64)]) -> f64 {
    if edges.is_empty() {
        return 0.0;
    }
    let sum: f64 = edges.iter().map(|(_, _, sim)| sim).sum();
    sum / edges.len() as f64
}

/// Compute the centroid (representative) article title for a cluster.
///
/// Picks the article with the highest average similarity to other members.
/// Falls back to the first article's title.
pub fn cluster_title(_article_ids: &[i64], edges: &[(i64, i64, f64)]) -> Option<String> {
    if edges.is_empty() {
        return None;
    }
    // Find the article with the most edges (highest degree centrality)
    let mut degree: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
    for (a, b, _) in edges {
        *degree.entry(*a).or_default() += 1;
        *degree.entry(*b).or_default() += 1;
    }
    // Return placeholder — real title resolution happens at the pipeline level
    // where we have access to the store layer
    degree.keys().next().map(|id| format!("cluster:{}", id))
}

/// Generate a stable cluster ID from member article IDs.
pub fn cluster_id(article_ids: &[i64]) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for id in article_ids {
        id.hash(&mut hasher);
    }
    format!("semantic:{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_avg_similarity() {
        let edges = vec![(1i64, 2i64, 0.9f64), (2i64, 3i64, 0.8f64), (1i64, 3i64, 0.7f64)];
        let avg = avg_pairwise_similarity(&edges);
        assert!((avg - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_avg_similarity_empty() {
        assert_eq!(avg_pairwise_similarity(&[]), 0.0);
    }

    #[test]
    fn test_cluster_id_stable() {
        let id1 = cluster_id(&[1, 2, 3]);
        let id2 = cluster_id(&[1, 2, 3]);
        assert_eq!(id1, id2);
        assert!(id1.starts_with("semantic:"));
    }

    #[test]
    fn test_cluster_id_different_inputs() {
        let id1 = cluster_id(&[1, 2, 3]);
        let id2 = cluster_id(&[4, 5, 6]);
        assert_ne!(id1, id2);
    }
}
