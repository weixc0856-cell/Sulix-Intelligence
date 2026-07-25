//! Signal Source trait — extension point for signal candidate providers.
//!
//! Each source produces `SignalCandidate` values that the engine persists
//! and merges.  Currently two sources:
//!
//! - [`EntitySignalSource`] — the original entity-driven engine
//! - [`SemanticDiscoverySource`] — ANN + clustering + V2 scoring

use store::{BriefArticle, DiscoveryMethod, RelatedEntityRef, StoreBackend};

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
        store: &'a dyn StoreBackend,
        now: i64,
    ) -> futures::future::LocalBoxFuture<'a, Result<Vec<SignalCandidate>, String>>;
}

/// Entity-driven signal source — the original engine.
pub struct EntitySignalSource;

impl SignalSource for EntitySignalSource {
    fn candidates<'a>(
        &'a self,
        store: &'a dyn StoreBackend,
        now: i64,
    ) -> futures::future::LocalBoxFuture<'a, Result<Vec<SignalCandidate>, String>> {
        Box::pin(async move {
            let rows = store
                .entity_signal_candidates_filtered(now, 7, 50, 3, 2)
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
                        evidence: c
                            .evidence
                            .into_iter()
                            .map(|e| BriefArticle {
                                id: e.id,
                                title: e.title,
                                url: e.url,
                                feed_name: e.feed_name,
                                score: e.score,
                            })
                            .collect(),
                        related_entities: related,
                    }
                })
                .collect())
        })
    }
}
