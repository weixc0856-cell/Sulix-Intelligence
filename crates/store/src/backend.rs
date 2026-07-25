//! `StoreBackend` trait — abstraction over D1 so the pipeline can be
//! unit-tested with a [`MemoryStore`](crate::memory::MemoryStore).
//!
//! MVP scope: only the methods used by the feed processing pipeline.

use async_trait::async_trait;

use crate::{
    ArtifactEntry, EntityActivitySummary, EntityArticle, EntityDetail, EntityRef, EntitySignalCandidate, EntitySummary,
    Feed, IntelligenceSignal, NewArtifact, NewArticle, RelatedEntity, SignalBriefInput, StoreError,
};

/// Storage backend for the feed pipeline.
///
/// Every method maps 1:1 to a D1 query.  The production implementation
/// ([`D1Store`](crate::D1Store)) wraps `worker::D1Database`; the test
/// implementation ([`MemoryStore`](crate::memory::MemoryStore)) uses
/// in-memory `HashMap`/`Vec` and supports failure injection.
#[async_trait(?Send)]
pub trait StoreBackend {
    // ---- Feeds ----

    /// Load one feed by id.
    async fn get_feed(&self, id: i64) -> Result<Option<Feed>, StoreError>;

    /// Record a fetch result (etag / last-modified) after a successful fetch.
    async fn record_fetch_result(
        &self,
        feed_id: i64,
        fetched_at: i64,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<(), StoreError>;

    // ---- Rules ----

    /// Return `rule_json` strings for every enabled rule matching `audience_tag`.
    async fn active_rule_jsons(&self, audience_tag: &str) -> Result<Vec<String>, StoreError>;

    // ---- Articles ----

    /// Insert a new article (INSERT OR IGNORE).  Returns the new row id,
    /// or `None` when the article already exists (duplicate GUID).
    async fn insert_article(&self, article: &NewArticle) -> Result<Option<i64>, StoreError>;

    /// Persist AI summarisation results.
    async fn set_ai_summary(
        &self,
        article_id: i64,
        summary: &str,
        tags_json: &str,
        vector_id: &str,
        score: f64,
    ) -> Result<(), StoreError>;

    /// Update the R2 key pointing to the article's full-text body.
    async fn set_raw_content_r2_key(
        &self,
        article_id: i64,
        r2_key: Option<&str>,
    ) -> Result<(), StoreError>;

    /// Delete articles older than `days` whose AI processing is complete.
    async fn expire_old_articles(&self, now: i64, days: i64) -> Result<u64, StoreError>;

    // ===== Intelligence / Entity methods =====

    /// Upsert an entity by normalized_name. Returns the entity id.
    async fn upsert_entity(&self, name: &str, normalized: &str, entity_type: &str) -> Result<i64, StoreError>;

    /// Link an article to an entity (many-to-many).
    async fn link_article_entity(
        &self,
        article_id: i64,
        entity_id: i64,
        relevance: f64,
        context: Option<&str>,
    ) -> Result<(), StoreError>;

    /// Link two entities with a directed relation.
    async fn link_entity_relation(
        &self,
        source: i64,
        target: i64,
        rtype: &str,
        confidence: f64,
    ) -> Result<(), StoreError>;

    /// List all entities, paginated, ordered by article_count DESC.
    async fn list_entities(&self, limit: u32, offset: u32) -> Result<Vec<EntitySummary>, StoreError>;

    /// Get a single entity by id with aggregate article_count.
    async fn entity_detail(&self, id: i64) -> Result<Option<EntityDetail>, StoreError>;

    /// Get related entities for a given entity through entity_relations.
    async fn entity_relations(&self, entity_id: i64, limit: u32) -> Result<Vec<RelatedEntity>, StoreError>;

    /// Get all entities linked to a specific article.
    async fn article_entities(&self, article_id: i64) -> Result<Vec<EntityRef>, StoreError>;

    /// Register an R2 artifact in the artifact_registry.
    async fn create_artifact(&self, artifact: &NewArtifact) -> Result<i64, StoreError>;

    /// List artifact_registry entries for a given entity.
    async fn list_artifacts_by_entity(&self, entity_id: i64, limit: u32) -> Result<Vec<ArtifactEntry>, StoreError>;

    // ===== Entity Intelligence methods =====

    /// List articles linked to an entity (Evidence).
    async fn entity_articles(
        &self,
        entity_id: i64,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<EntityArticle>, StoreError>;

    /// Activity summary for an entity over the last N days.
    async fn entity_activity_summary(
        &self,
        entity_id: i64,
        now: i64,
        days: i64,
    ) -> Result<EntityActivitySummary, StoreError>;

    /// Generate entity-anchored signal candidates with 5-factor scoring.
    async fn entity_signal_candidates(
        &self,
        now: i64,
        days: i64,
        limit: u32,
    ) -> Result<Vec<EntitySignalCandidate>, StoreError>;

    // ===== Signal Persistence =====

    /// Persist an intelligence signal with evidence and entity links.
    #[allow(clippy::too_many_arguments)]
    async fn save_signal(
        &self,
        entity_id: Option<i64>,
        title: &str,
        summary: &str,
        confidence: f64,
        impact: &str,
        trend: &str,
        article_count: i64,
        source_count: i64,
        evidence_ids: &[i64],
        related_ids: &[i64],
    ) -> Result<i64, StoreError>;

    /// Load recent intelligence signals.
    async fn load_recent_signals(&self, limit: u32, offset: u32) -> Result<Vec<IntelligenceSignal>, StoreError>;

    /// Load a single signal by id.
    async fn load_signal_by_id(&self, id: i64) -> Result<Option<IntelligenceSignal>, StoreError>;

    /// Load signals anchored to a specific entity.
    async fn entity_signals(&self, entity_id: i64, limit: u32) -> Result<Vec<IntelligenceSignal>, StoreError>;

    async fn upsert_signal_thread(
        &self,
        signal_key: &str,
        anchor_entity_id: Option<i64>,
        title: &str,
        status: &str,
    ) -> Result<i64, StoreError>;

    async fn append_signal_instance(
        &self,
        thread_id: i64,
        confidence: f64,
        impact: &str,
        trend: &str,
        article_count: i64,
        source_count: i64,
    ) -> Result<i64, StoreError>;

    async fn update_signal_lifecycle(&self, now: i64) -> Result<(), StoreError>;

    async fn get_active_signal_threads(&self, limit: u32) -> Result<Vec<SignalBriefInput>, StoreError>;
}
