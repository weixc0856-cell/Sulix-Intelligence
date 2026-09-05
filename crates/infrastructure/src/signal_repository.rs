//! Store-backed [`SignalPersistence`] / [`SignalDiscovery`] / [`SignalQuery`] adapters.
//!
//! Bridges the signal-engine write-orchestration + candidate-discovery ports
//! onto the D1 store (fine-grained `SignalStore`/`SignalQueryService`/… traits,
//! never the legacy `StoreBackend`). Lives in infrastructure so signal-engine
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
use signal_engine::models::{
    BriefArticle, DiscoveryMethod, EmbeddedArticle, EntityCandidate, EntitySignalRef, HealthComponents,
    RelatedEntityRef, RelatedSignalRef, SignalAnalysis, SignalDetail, SignalEventRecord, SignalHealthDetail2,
    SignalInstanceMoment, SignalMutation, SignalThreadFilter, SignalThreadRow, SignalTimelineEvent, SignalUpsertResult,
};
use signal_engine::ports::{SignalDiscovery, SignalPersistence, SignalQuery};
use store::{
    ArticleEmbeddingRef, ArticleQueryService, EntitySignalCandidate, SignalBriefInput, SignalEvent, SignalQueryService,
    SignalStore, StoreError,
};

/// Error-map `store::StoreError` → domain `SignalError::Persistence`.
fn to_persistence(e: StoreError) -> SignalError {
    SignalError::Persistence(e.to_string())
}

/// Error-map `store::StoreError` → domain `SignalError::Discovery`.
fn to_discovery(e: StoreError) -> SignalError {
    SignalError::Discovery(e.to_string())
}

/// Error-map `store::StoreError` → domain `SignalError::Query`.
fn to_query(e: StoreError) -> SignalError {
    SignalError::Query(e.to_string())
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
        entity_type: c.entity_type,
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

impl<'a, S: SignalStore> D1SignalPersistence<'a, S> {
    pub fn new(store: &'a S) -> Self {
        Self { store }
    }
}

#[async_trait(?Send)]
impl<S: SignalStore> SignalPersistence for D1SignalPersistence<'_, S> {
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

impl<'a, S: SignalStore + ArticleQueryService> D1SignalDiscovery<'a, S> {
    pub fn new(store: &'a S) -> Self {
        Self { store }
    }
}

#[async_trait(?Send)]
impl<S: SignalStore + ArticleQueryService> SignalDiscovery for D1SignalDiscovery<'_, S> {
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

// ---- Read-model mapping (store DTO → owned SignalQuery records) ----

/// Map a store `BriefArticle` DTO → the owned brief article record.
fn to_owned_brief(b: store::BriefArticle) -> BriefArticle {
    BriefArticle { id: b.id, title: b.title, url: b.url, feed_name: b.feed_name, score: b.score }
}

/// Map a store `RelatedEntityRef` DTO → the owned record.
fn to_owned_related(r: store::RelatedEntityRef) -> RelatedEntityRef {
    RelatedEntityRef {
        id: r.id,
        name: r.name,
        entity_type: r.entity_type,
        relation_type: r.relation_type,
        relation: r.relation,
        confidence: r.confidence,
    }
}

/// Map a store `EntitySignalRef` DTO → the owned record.
fn to_owned_entity_ref(r: store::EntitySignalRef) -> EntitySignalRef {
    EntitySignalRef { id: r.id, name: r.name, entity_type: r.entity_type }
}

/// Map a store `RelatedSignalRef` DTO → the owned record.
fn to_owned_related_signal(r: store::RelatedSignalRef) -> RelatedSignalRef {
    RelatedSignalRef { id: r.id, title: r.title, status: r.status, health_score: r.health_score }
}

/// Map a store `SignalHealthDetail2` DTO → the owned record.
fn to_owned_health(h: store::SignalHealthDetail2) -> SignalHealthDetail2 {
    SignalHealthDetail2 {
        score: h.score,
        components: HealthComponents {
            volume: h.components.volume,
            diversity: h.components.diversity,
            quality: h.components.quality,
            velocity: h.components.velocity,
            persistence: h.components.persistence,
        },
    }
}

/// Map a store `SignalTimelineEvent` DTO → the owned record.
fn to_owned_timeline(t: store::SignalTimelineEvent) -> SignalTimelineEvent {
    SignalTimelineEvent {
        timestamp: t.timestamp,
        event_type: t.event_type,
        score: t.score,
        article_count: t.article_count,
        description: t.description,
    }
}

/// Map a store `SignalAnalysis` DTO → the owned record.
fn to_owned_analysis(a: store::SignalAnalysis) -> SignalAnalysis {
    SignalAnalysis { why_it_matters: a.why_it_matters, impact: a.impact, confidence_reason: a.confidence_reason }
}

/// Map a store `SignalDetail` DTO → the owned read model (serde-mirror shape).
fn to_owned_detail(d: store::SignalDetail) -> SignalDetail {
    SignalDetail {
        id: d.id,
        title: d.title,
        description: d.description,
        status: d.status,
        trend: d.trend,
        health: to_owned_health(d.health),
        anchor_entity: d.anchor_entity.map(to_owned_entity_ref),
        first_seen_at: d.first_seen_at,
        last_seen_at: d.last_seen_at,
        timeline: d.timeline.into_iter().map(to_owned_timeline).collect(),
        evidence_top: d.evidence_top.into_iter().map(to_owned_brief).collect(),
        related_entities: d.related_entities.into_iter().map(to_owned_related).collect(),
        related_signals: d.related_signals.into_iter().map(to_owned_related_signal).collect(),
        analysis: d.analysis.map(to_owned_analysis),
    }
}

/// Map a store `SignalEvent` row → the owned stored-event record (trimmed).
fn to_owned_event_record(e: SignalEvent) -> SignalEventRecord {
    SignalEventRecord { event_type: e.event_type, payload: e.payload, created_at: e.created_at }
}

/// Map the owned listing filter → the store `SignalThreadFilter`.
fn to_store_filter(f: &SignalThreadFilter) -> store::SignalThreadFilter {
    store::SignalThreadFilter { statuses: f.statuses.clone(), limit: f.limit, min_score: f.min_score }
}

/// Map a store `SignalBriefInput` listing DTO → the owned listing projection.
fn to_owned_thread_row(t: SignalBriefInput) -> SignalThreadRow {
    SignalThreadRow {
        thread_id: t.thread_id,
        signal_key: t.signal_key,
        anchor_entity: t.anchor_entity,
        title: t.title,
        status: t.status,
        health_score: t.health_score,
        trend: t.trend,
        current_score: t.current_score,
        cumulative_article_count: t.cumulative_article_count,
        instances: t.instances.into_iter().map(|i| SignalInstanceMoment { generated_at: i.generated_at }).collect(),
    }
}

/// Read-model adapter (thread detail / stored events / thread listing).
pub struct D1SignalQuery<'a, S> {
    store: &'a S,
}

impl<'a, S: SignalStore + SignalQueryService> D1SignalQuery<'a, S> {
    pub fn new(store: &'a S) -> Self {
        Self { store }
    }
}

#[async_trait(?Send)]
impl<S: SignalStore + SignalQueryService> SignalQuery for D1SignalQuery<'_, S> {
    async fn load_signal_detail(&self, thread_id: i64) -> Result<Option<SignalDetail>, SignalError> {
        let d = self.store.load_signal_detail(thread_id).await.map_err(to_query)?;
        Ok(d.map(to_owned_detail))
    }

    async fn load_signal_events(&self, thread_id: i64, limit: u32) -> Result<Vec<SignalEventRecord>, SignalError> {
        let rows = self.store.load_signal_events(thread_id, limit).await.map_err(to_query)?;
        Ok(rows.into_iter().map(to_owned_event_record).collect())
    }

    async fn list_signal_threads(&self, filter: &SignalThreadFilter) -> Result<Vec<SignalThreadRow>, SignalError> {
        let rows = self.store.list_signal_threads(&to_store_filter(filter)).await.map_err(to_query)?;
        Ok(rows.into_iter().map(to_owned_thread_row).collect())
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
        assert_eq!(owned.entity_type, "product");
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

    // ---- Read-model DTO → owned mapping ----

    fn detail_dto() -> store::SignalDetail {
        store::SignalDetail {
            id: 9,
            title: "CUDA momentum".into(),
            description: "desc".into(),
            status: "active".into(),
            trend: "rising".into(),
            health: store::SignalHealthDetail2 {
                score: 0.72,
                components: store::HealthComponents {
                    volume: 0.8,
                    diversity: 0.6,
                    quality: 0.7,
                    velocity: 0.9,
                    persistence: 0.4,
                },
            },
            anchor_entity: Some(store::EntitySignalRef { id: 7, name: "CUDA".into(), entity_type: "product".into() }),
            first_seen_at: 1_000,
            last_seen_at: 2_000,
            timeline: vec![store::SignalTimelineEvent {
                timestamp: 2_000,
                event_type: "score_changed".into(),
                score: 0.72,
                article_count: 5,
                description: "Score changed to 0.7 (5 articles)".into(),
            }],
            evidence_top: vec![store::BriefArticle {
                id: 11,
                title: "CUDA adoption".into(),
                url: Some("https://example.com/cuda".into()),
                feed_name: Some("GPU Weekly".into()),
                score: 0.81,
            }],
            related_entities: vec![store::RelatedEntityRef {
                id: 2,
                name: "NVIDIA".into(),
                entity_type: "organization".into(),
                relation_type: "parent".into(),
                relation: Some("parent of".into()),
                confidence: Some(0.9),
            }],
            related_signals: vec![store::RelatedSignalRef {
                id: 3,
                title: "NVIDIA outlook".into(),
                status: "active".into(),
                health_score: 0.6,
            }],
            analysis: Some(store::SignalAnalysis {
                why_it_matters: "matters".into(),
                impact: "High".into(),
                confidence_reason: "5 articles".into(),
            }),
        }
    }

    #[test]
    fn owned_detail_maps_full_dto_faithfully() {
        let owned = to_owned_detail(detail_dto());
        assert_eq!(owned.id, 9);
        assert_eq!(owned.trend, "rising");
        assert_eq!(owned.health.score, 0.72);
        assert_eq!(owned.health.components.velocity, 0.9);
        assert_eq!(owned.anchor_entity.as_ref().unwrap().name, "CUDA");
        assert_eq!(owned.timeline[0].article_count, 5);
        assert_eq!(owned.evidence_top[0].title, "CUDA adoption");
        assert_eq!(owned.related_entities[0].relation_type, "parent");
        assert_eq!(owned.related_signals[0].health_score, 0.6);
        assert_eq!(owned.analysis.as_ref().unwrap().impact, "High");
    }

    #[test]
    fn owned_detail_json_matches_store_dto() {
        // The owned read model is a serde mirror: serialising an identically
        // valued owned SignalDetail must produce the same JSON as the store DTO.
        let store_dto = detail_dto();
        let owned = to_owned_detail(store_dto.clone());
        assert_eq!(serde_json::to_value(&owned).unwrap(), serde_json::to_value(&store_dto).unwrap());
    }

    #[test]
    fn stored_event_record_trims_dto() {
        let e = store::SignalEvent {
            id: 1,
            thread_id: 9,
            event_type: "score_changed".into(),
            payload: Some(r#"{"score":0.72}"#.into()),
            created_at: 2_000,
        };
        let owned = to_owned_event_record(e);
        assert_eq!(owned.event_type, "score_changed");
        assert_eq!(owned.created_at, 2_000);
        assert!(owned.payload.unwrap().contains("0.72"));
    }

    #[test]
    fn thread_row_projects_brief_input() {
        let row = store::SignalBriefInput {
            thread_id: 9,
            signal_key: "entity:7".into(),
            anchor_entity: Some("CUDA".into()),
            title: "CUDA momentum".into(),
            description: String::new(),
            status: "active".into(),
            health_score: 0.72,
            current_score: 0.72,
            trend: "rising".into(),
            cumulative_article_count: 5,
            recent_article_count: 3,
            source_count: 2,
            velocity: 1.2,
            instances: vec![
                store::SignalInstanceSummary {
                    id: 1,
                    score: 0.7,
                    confidence: 0.6,
                    trend: "rising".into(),
                    article_count: 5,
                    source_count: 2,
                    generated_at: 1_000,
                },
                store::SignalInstanceSummary {
                    id: 2,
                    score: 0.72,
                    confidence: 0.6,
                    trend: "rising".into(),
                    article_count: 6,
                    source_count: 2,
                    generated_at: 2_000,
                },
            ],
            evidence: vec![],
            related_entities: vec![],
            provenance: store::SignalProvenance { method: store::DiscoveryMethod::Entity, score: Some(0.72) },
        };
        let owned = to_owned_thread_row(row);
        assert_eq!(owned.signal_key, "entity:7");
        assert_eq!(owned.anchor_entity.as_deref(), Some("CUDA"));
        assert_eq!(owned.current_score, 0.72);
        assert_eq!(owned.cumulative_article_count, 5);
        assert_eq!(owned.instances.len(), 2);
        assert_eq!(owned.instances[0].generated_at, 1_000);
        assert_eq!(owned.instances[1].generated_at, 2_000);
    }
}
