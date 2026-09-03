//! Store-backed [`SignalPersistence`] / [`SignalDiscovery`] adapters.
//!
//! Bridges the signal-engine write-orchestration + candidate-discovery ports
//! onto the D1 store (`StoreBackend`). Lives in infrastructure so signal-engine
//! never depends on store.
//!
//! Unlike Round-1 adapters (owned `store: S`), these hold the store **by
//! reference** (`&'a S`): `MemoryStore`/`D1Store` are not `Clone`, and the
//! persistence + discovery adapters must share a single store instance for the
//! shared-state infra tests (and the same D1 binding in the worker). That is
//! the sole deviation from the owned-S convention — see module docs of
//! `context_repository` for the Round-1 pattern.

use async_trait::async_trait;
use signal_engine::error::SignalError;
use signal_engine::models::{DiscoveryMethod, EmbeddedArticle, EntityCandidate, SignalMutation, SignalUpsertResult};
use signal_engine::ports::{SignalDiscovery, SignalPersistence};
use store::{ArticleEmbeddingRef, EntitySignalCandidate, StoreBackend, StoreError};

/// Error-map `store::StoreError` → domain `SignalError::Persistence`.
fn to_persistence(e: StoreError) -> SignalError {
    SignalError::Persistence(e.to_string())
}

/// Error-map `store::StoreError` → domain `SignalError::Discovery`.
fn to_discovery(e: StoreError) -> SignalError {
    SignalError::Discovery(e.to_string())
}

fn to_store_method(m: DiscoveryMethod) -> store::DiscoveryMethod {
    match m {
        DiscoveryMethod::Entity => store::DiscoveryMethod::Entity,
        DiscoveryMethod::Semantic => store::DiscoveryMethod::Semantic,
        DiscoveryMethod::Hybrid => store::DiscoveryMethod::Hybrid,
    }
}

fn to_owned_result(r: store::SignalUpsertResult) -> SignalUpsertResult {
    SignalUpsertResult {
        id: r.id,
        mutation: match r.mutation {
            store::SignalMutation::Created => SignalMutation::Created,
            store::SignalMutation::Updated => SignalMutation::Updated,
        },
    }
}

/// Map a store `EntitySignalCandidate` DTO → the trimmed owned projection.
fn to_owned_candidate(c: EntitySignalCandidate) -> EntityCandidate {
    EntityCandidate {
        entity_id: c.entity_id,
        entity_name: c.entity_name,
        score: c.score,
        trend: c.trend,
        article_count: c.article_count,
        source_count: c.source_count,
        avg_score: c.avg_score,
        // Store evidence rows carry `published_at`; the owned brief article
        // drops it (matching the previous source.rs projection).
        evidence: c
            .evidence
            .into_iter()
            .map(|e| signal_engine::models::BriefArticle {
                id: e.id,
                title: e.title,
                url: e.url,
                feed_name: e.feed_name,
                score: e.score,
            })
            .collect(),
        related_entity_ids: c.related_entity_ids,
    }
}

/// Map a store `ArticleEmbeddingRef` DTO → the trimmed owned record.
fn to_owned_embedded(e: ArticleEmbeddingRef) -> EmbeddedArticle {
    EmbeddedArticle { article_id: e.article_id, vector_id: e.vector_id }
}

/// Write-orchestration adapter (thread upsert / instance append / lifecycle).
pub struct D1SignalPersistence<'a, S> {
    store: &'a S,
}

impl<'a, S: StoreBackend> D1SignalPersistence<'a, S> {
    pub fn new(store: &'a S) -> Self {
        Self { store }
    }
}

#[async_trait(?Send)]
impl<S: StoreBackend> SignalPersistence for D1SignalPersistence<'_, S> {
    async fn upsert_signal_thread(
        &self,
        signal_key: &str,
        anchor_entity_id: Option<i64>,
        title: &str,
        status: &str,
        discovery_method: &DiscoveryMethod,
        discovery_score: Option<f64>,
    ) -> Result<SignalUpsertResult, SignalError> {
        let r = self
            .store
            .upsert_signal_thread(
                signal_key,
                anchor_entity_id,
                title,
                status,
                &to_store_method(*discovery_method),
                discovery_score,
            )
            .await
            .map_err(to_persistence)?;
        Ok(to_owned_result(r))
    }

    async fn latest_instance_fingerprint(&self, thread_id: i64) -> Result<Option<(f64, String)>, SignalError> {
        self.store.get_latest_instance_fingerprint(thread_id).await.map_err(to_persistence)
    }

    async fn append_signal_instance(
        &self,
        thread_id: i64,
        score: f64,
        impact: &str,
        trend: &str,
        article_count: i64,
        source_count: i64,
        avg_score: f64,
        entity_id: i64,
    ) -> Result<i64, SignalError> {
        self.store
            .append_signal_instance_v2(
                thread_id,
                score,
                impact,
                trend,
                article_count,
                source_count,
                avg_score,
                entity_id,
            )
            .await
            .map_err(to_persistence)
    }

    async fn update_signal_lifecycle(&self, now: i64) -> Result<(), SignalError> {
        self.store.update_signal_lifecycle(now).await.map_err(to_persistence)
    }
}

/// Candidate-discovery adapter (entity candidates / embedded articles).
pub struct D1SignalDiscovery<'a, S> {
    store: &'a S,
}

impl<'a, S: StoreBackend> D1SignalDiscovery<'a, S> {
    pub fn new(store: &'a S) -> Self {
        Self { store }
    }
}

#[async_trait(?Send)]
impl<S: StoreBackend> SignalDiscovery for D1SignalDiscovery<'_, S> {
    async fn entity_signal_candidates(
        &self,
        now: i64,
        days: i64,
        limit: u32,
        min_entity_articles: u32,
        min_sources: u32,
    ) -> Result<Vec<EntityCandidate>, SignalError> {
        let rows = self
            .store
            .entity_signal_candidates_filtered(now, days, limit, min_entity_articles, min_sources)
            .await
            .map_err(to_discovery)?;
        Ok(rows.into_iter().map(to_owned_candidate).collect())
    }

    async fn recent_embedded_articles(
        &self,
        now: i64,
        days: i64,
        limit: u32,
    ) -> Result<Vec<EmbeddedArticle>, SignalError> {
        let rows = self.store.recent_embedded_articles(now, days, limit).await.map_err(to_discovery)?;
        Ok(rows.into_iter().map(to_owned_embedded).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signal_engine::models::BriefArticle;
    use store::memory::MemoryStore;

    /// MemoryStore returns hardcoded `Created`/`id=1` upserts — verify the
    /// adapter maps the store DTO → owned record faithfully.
    #[test]
    fn persistence_adapter_maps_upsert_over_memory_store() {
        let store = MemoryStore::new();
        let p = D1SignalPersistence::new(&store);
        let r = futures::executor::block_on(p.upsert_signal_thread(
            "entity:1",
            Some(1),
            "NVIDIA",
            "active",
            &DiscoveryMethod::Entity,
            Some(0.8),
        ))
        .unwrap();
        assert_eq!(r.id, 1);
        assert_eq!(r.mutation, SignalMutation::Created);
    }

    #[test]
    fn discovery_adapter_maps_embedded_articles_over_memory_store() {
        let store = MemoryStore::new();
        let d = D1SignalDiscovery::new(&store);
        // MemoryStore's recent_embedded_articles returns empty — the mapping is
        // exercised on fabricated DTOs below; this verifies the call path only.
        let rows = futures::executor::block_on(d.recent_embedded_articles(1_000_000, 7, 200)).unwrap();
        assert!(rows.is_empty());
    }

    // ---- Pure DTO → owned mapping ----

    fn candidate_dto() -> EntitySignalCandidate {
        EntitySignalCandidate {
            entity_id: 7,
            entity_name: "CUDA".into(),
            entity_type: "product".into(),
            score: 0.9,
            volume: 1.0,
            diversity: 0.5,
            quality: 0.9,
            velocity: 0.7,
            novelty: 0.4,
            article_count: 5,
            source_count: 3,
            avg_score: 0.6,
            trend: "rising".into(),
            evidence: vec![store::SignalEvidence {
                id: 11,
                title: "CUDA adoption".into(),
                url: Some("https://example.com/cuda".into()),
                feed_name: Some("GPU Weekly".into()),
                published_at: Some(999_000),
                score: 0.81,
            }],
            related_entity_ids: vec![2, 3],
        }
    }

    #[test]
    fn owned_candidate_trims_dto_faithfully() {
        let owned = to_owned_candidate(candidate_dto());
        assert_eq!(owned.entity_id, 7);
        assert_eq!(owned.entity_name, "CUDA");
        assert_eq!(owned.score, 0.9);
        assert_eq!(owned.trend, "rising");
        assert_eq!(owned.article_count, 5);
        assert_eq!(owned.source_count, 3);
        assert_eq!(owned.avg_score, 0.6);
        assert_eq!(owned.related_entity_ids, vec![2, 3]);
        assert_eq!(
            owned.evidence,
            vec![BriefArticle {
                id: 11,
                title: "CUDA adoption".into(),
                url: Some("https://example.com/cuda".into()),
                feed_name: Some("GPU Weekly".into()),
                score: 0.81,
            }]
        );
    }

    #[test]
    fn owned_embedded_trims_dto() {
        let owned = to_owned_embedded(ArticleEmbeddingRef {
            article_id: 42,
            vector_id: "article-42".into(),
            published_at: 999_000,
            source_id: 3,
            entity_ids: vec![7],
        });
        assert_eq!(owned.article_id, 42);
        assert_eq!(owned.vector_id, "article-42");
    }

    #[test]
    fn discovery_method_roundtrips_to_store() {
        // store::DiscoveryMethod derives only Debug/Clone/Serde (no PartialEq) → matches!
        assert!(matches!(to_store_method(DiscoveryMethod::Entity), store::DiscoveryMethod::Entity));
        assert!(matches!(to_store_method(DiscoveryMethod::Semantic), store::DiscoveryMethod::Semantic));
        assert!(matches!(to_store_method(DiscoveryMethod::Hybrid), store::DiscoveryMethod::Hybrid));
    }
}
