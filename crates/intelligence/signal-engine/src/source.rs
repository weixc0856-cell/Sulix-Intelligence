//! Signal Source trait — extension point for signal candidate providers.
//!
//! Each source produces `SignalCandidate` values that the engine persists
//! and merges.
//!
//! - [`EntitySignalSource`] — entity-driven signal candidates
//! - [`SemanticDiscoverySource`] — ANN + clustering + V2 scoring (needs Vectorize)

use crate::discovery::clustering::{cluster_by_similarity, SimilarityEdge};
use crate::discovery::converter::cluster_to_candidate;
use crate::models::{BriefArticle, DiscoveryMethod, RelatedEntityRef};
use crate::ports::{SemanticQuery, SignalDiscovery};

/// Runtime context provided to every [`SignalSource`].
#[derive(Clone)]
pub struct DiscoveryContext<'a> {
    /// Candidate-discovery boundary (entity candidates / embedded articles).
    pub discovery: &'a dyn SignalDiscovery,
    pub semantic: Option<&'a dyn SemanticQuery>,
    pub now: i64,
}

/// A candidate signal before materialisation into a thread.
#[derive(Debug, Clone)]
pub struct SignalCandidate {
    pub signal_key: String,
    pub anchor_entity_id: Option<i64>,
    pub title: String,
    pub status: String,
    pub discovery_method: DiscoveryMethod,
    pub discovery_score: Option<f64>,
    pub score: f64,
    pub trend: String,
    pub article_count: i64,
    pub source_count: i64,
    pub avg_score: f64,
    pub evidence: Vec<BriefArticle>,
    pub related_entities: Vec<RelatedEntityRef>,
}

/// A source of signal candidates.
pub trait SignalSource {
    fn candidates<'a>(
        &'a self,
        ctx: DiscoveryContext<'a>,
    ) -> futures::future::LocalBoxFuture<'a, Result<Vec<SignalCandidate>, String>>;
}

/// Entity-driven signal source — the original engine.
pub struct EntitySignalSource;

impl SignalSource for EntitySignalSource {
    fn candidates<'a>(
        &'a self,
        ctx: DiscoveryContext<'a>,
    ) -> futures::future::LocalBoxFuture<'a, Result<Vec<SignalCandidate>, String>> {
        Box::pin(async move {
            let rows = ctx
                .discovery
                .entity_signal_candidates(ctx.now, 7, 50, 3, 2)
                .await
                .map_err(|e| format!("entity_candidates failed: {e}"))?;

            Ok(rows
                .into_iter()
                .map(|c| {
                    let related: Vec<RelatedEntityRef> = c
                        .related_entity_ids
                        .iter()
                        .map(|&id| RelatedEntityRef {
                            id,
                            name: String::new(),
                            entity_type: String::new(),
                            relation_type: String::new(),
                            relation: None,
                            confidence: None,
                        })
                        .collect();
                    SignalCandidate {
                        signal_key: format!("entity:{}", c.entity_id),
                        anchor_entity_id: Some(c.entity_id),
                        title: c.entity_name.clone(),
                        status: "active".into(),
                        discovery_method: DiscoveryMethod::Entity,
                        discovery_score: Some(c.score),
                        score: c.score,
                        trend: c.trend,
                        article_count: c.article_count,
                        source_count: c.source_count,
                        avg_score: c.avg_score,
                        evidence: c.evidence,
                        related_entities: related,
                    }
                })
                .collect())
        })
    }
}

/// Semantic discovery signal source — ANN similarity + clustering + V2 scoring.
///
/// Requires a [`SemanticQuery`] (Vectorize-backed in production) to be present
/// in [`DiscoveryContext`]. When it is unavailable, returns empty (no candidates).
pub struct SemanticDiscoverySource;

impl SignalSource for SemanticDiscoverySource {
    fn candidates<'a>(
        &'a self,
        ctx: DiscoveryContext<'a>,
    ) -> futures::future::LocalBoxFuture<'a, Result<Vec<SignalCandidate>, String>> {
        Box::pin(async move {
            let semantic = match ctx.semantic {
                Some(s) => s,
                None => return Ok(Vec::new()),
            };

            // 1. Load recent articles with embeddings
            let articles = ctx
                .discovery
                .recent_embedded_articles(ctx.now, 7, 200)
                .await
                .map_err(|e| format!("recent_embedded_articles failed: {e}"))?;

            if articles.len() < 3 {
                return Ok(Vec::new());
            }

            // 2. Build similarity edges via semantic ANN
            let mut edges: Vec<SimilarityEdge> = Vec::new();
            let mut seen: std::collections::HashSet<(i64, i64)> = std::collections::HashSet::new();

            for a in &articles {
                let matches = semantic
                    .find_similar(&a.vector_id, 20, 0.75)
                    .await
                    .map_err(|e| format!("semantic query for {}: {e}", a.vector_id))?;

                for m in &matches {
                    if let Ok(neighbor_id) = m.vector_id.parse::<i64>() {
                        if neighbor_id != a.article_id {
                            let key = if a.article_id < neighbor_id {
                                (a.article_id, neighbor_id)
                            } else {
                                (neighbor_id, a.article_id)
                            };
                            if seen.insert(key) {
                                edges.push(SimilarityEdge {
                                    article_a: a.article_id,
                                    article_b: neighbor_id,
                                    similarity: m.score,
                                });
                            }
                        }
                    }
                }
            }

            // 3. Cluster by similarity
            let article_ids: Vec<i64> = articles.iter().map(|a| a.article_id).collect();
            let clusters = cluster_by_similarity(&edges, &article_ids, 0.75, 3);

            // 4. Convert clusters to candidates
            let candidates: Vec<SignalCandidate> =
                clusters.iter().map(|c| cluster_to_candidate_vec(c, &edges, &article_ids)).collect();

            Ok(candidates)
        })
    }
}

fn cluster_to_candidate_vec(cluster: &[i64], edges: &[SimilarityEdge], _article_ids: &[i64]) -> SignalCandidate {
    let cluster_obj = crate::discovery::clustering::ArticleCluster { article_ids: cluster.to_vec() };
    cluster_to_candidate(&cluster_obj, edges)
}
