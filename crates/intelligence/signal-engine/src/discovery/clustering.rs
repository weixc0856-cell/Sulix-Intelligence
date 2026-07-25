//! Union-Find graph clustering — groups articles by embedding similarity edges.
//!
//! Two-stage approach:
//! 1. ANN retrieval (Vectorize) gives us similarity edges: (a, b, similarity)
//! 2. Union-Find merges connected components above `min_similarity` threshold
//!
//! This avoids O(n²) all-pairs similarity by using Vectorize's native ANN.

/// A similarity edge between two articles.
#[derive(Debug, Clone)]
pub struct SimilarityEdge {
    pub article_a: i64,
    pub article_b: i64,
    pub similarity: f64,
}

/// A cluster of articles discovered by graph connectivity.
#[derive(Debug, Clone)]
pub struct ArticleCluster {
    pub article_ids: Vec<i64>,
}

/// Union-Find data structure for efficient graph clustering.
struct UnionFind {
    parent: Vec<usize>,
    size: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self { parent: (0..n).collect(), size: vec![1; n] }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }

    fn union(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra != rb {
            if self.size[ra] < self.size[rb] {
                self.parent[ra] = rb;
                self.size[rb] += self.size[ra];
            } else {
                self.parent[rb] = ra;
                self.size[ra] += self.size[rb];
            }
        }
    }

    fn clusters(&mut self) -> Vec<Vec<usize>> {
        let mut map: std::collections::BTreeMap<usize, Vec<usize>> = std::collections::BTreeMap::new();
        for i in 0..self.parent.len() {
            let root = self.find(i); // This won't compress; that's fine for final read
            map.entry(root).or_default().push(i);
        }
        map.into_values().collect()
    }
}

/// Build article indices from a list of IDs.
fn build_index_map(article_ids: &[i64]) -> std::collections::HashMap<i64, usize> {
    article_ids.iter().enumerate().map(|(i, id)| (*id, i)).collect()
}

/// Cluster articles by similarity edges using Union-Find.
///
/// # Arguments
/// * `edges` - Similarity edges from Vectorize ANN retrieval
/// * `article_ids` - All article IDs in the candidate set
/// * `min_similarity` - Minimum similarity threshold (default 0.75)
/// * `min_cluster_size` - Minimum articles per cluster (default 3)
///
/// # Returns
/// A list of clusters, each containing article IDs sorted by ID.
pub fn cluster_by_similarity(
    edges: &[SimilarityEdge],
    article_ids: &[i64],
    min_similarity: f64,
    min_cluster_size: usize,
) -> Vec<Vec<i64>> {
    if article_ids.is_empty() {
        return Vec::new();
    }

    let index = build_index_map(article_ids);
    let mut uf = UnionFind::new(article_ids.len());

    // Union articles connected by edges above threshold
    for edge in edges {
        if edge.similarity < min_similarity {
            continue;
        }
        if let (Some(&a), Some(&b)) = (index.get(&edge.article_a), index.get(&edge.article_b)) {
            uf.union(a, b);
        }
    }

    // Convert back to article IDs, filter by minimum size
    let raw_clusters = uf.clusters();
    let mut result: Vec<Vec<i64>> = raw_clusters
        .into_iter()
        .filter(|c| c.len() >= min_cluster_size)
        .map(|c| {
            let mut ids: Vec<i64> = c.iter().map(|&i| article_ids[i]).collect();
            ids.sort_unstable();
            ids
        })
        .collect();

    // Sort clusters by size descending
    result.sort_by_key(|b| std::cmp::Reverse(b.len()));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_two_separate_clusters() {
        // Articles: [1, 2, 3, 4, 5, 6]
        // Edges: 1-2 (0.9), 2-3 (0.8), 4-5 (0.85)
        // Expect: [1,2,3] and [4,5] (6 is isolated)
        let edges = vec![
            SimilarityEdge { article_a: 1, article_b: 2, similarity: 0.9 },
            SimilarityEdge { article_a: 2, article_b: 3, similarity: 0.8 },
            SimilarityEdge { article_a: 4, article_b: 5, similarity: 0.85 },
        ];
        let ids = vec![1, 2, 3, 4, 5, 6];
        let clusters = cluster_by_similarity(&edges, &ids, 0.75, 2);
        assert_eq!(clusters.len(), 2, "should find 2 clusters");
        assert!(clusters.iter().any(|c| c == &vec![1, 2, 3]));
        assert!(clusters.iter().any(|c| c == &vec![4, 5]));
    }

    #[test]
    fn test_min_similarity_filter() {
        // Edge below threshold should be ignored
        let edges = vec![SimilarityEdge { article_a: 1, article_b: 2, similarity: 0.5 }];
        let ids = vec![1, 2, 3];
        let clusters = cluster_by_similarity(&edges, &ids, 0.75, 2);
        assert!(clusters.is_empty(), "no clusters should form");
    }

    #[test]
    fn test_min_cluster_size_filter() {
        let edges = vec![SimilarityEdge { article_a: 1, article_b: 2, similarity: 0.9 }];
        let ids = vec![1, 2, 3, 4];
        // min_cluster_size=3, cluster [1,2] is too small
        let clusters = cluster_by_similarity(&edges, &ids, 0.75, 3);
        assert!(clusters.is_empty(), "cluster too small");
    }

    #[test]
    fn test_empty_inputs() {
        assert!(cluster_by_similarity(&[], &[], 0.75, 3).is_empty());
    }

    #[test]
    fn test_single_cluster() {
        let edges = vec![
            SimilarityEdge { article_a: 1, article_b: 2, similarity: 0.9 },
            SimilarityEdge { article_a: 2, article_b: 3, similarity: 0.85 },
            SimilarityEdge { article_a: 3, article_b: 4, similarity: 0.8 },
        ];
        let ids = vec![1, 2, 3, 4];
        let clusters = cluster_by_similarity(&edges, &ids, 0.75, 2);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0], vec![1, 2, 3, 4]);
    }
}
