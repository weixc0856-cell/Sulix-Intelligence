//! Read-model queries for the Entity (Knowledge Graph) domain.
//!
//! Entity mutations (`save_entity`, `link_article`, `link_relation`)
//! belong in [`super::super::repo::EntityRepository`].
//! Signal-candidate generation methods (`entity_signal_candidates`,
//! `entity_signal_candidates_filtered`) remain on
//! [`StoreBackend`](crate::StoreBackend) until they are promoted to
//! the Intelligence context.

use async_trait::async_trait;

use crate::{EntityActivitySummary, EntityArticle, EntityDetail, EntitySummary, RelatedEntity, StoreError};

#[async_trait(?Send)]
pub trait EntityQueryService {
    /// List all entities, paginated, ordered by article_count DESC.
    async fn list_entities(&self, limit: u32, offset: u32) -> Result<Vec<EntitySummary>, StoreError>;

    /// Get a single entity by id (with aggregate article_count).
    async fn entity_detail(&self, id: i64) -> Result<Option<EntityDetail>, StoreError>;

    /// Get related entities via the entity_relations graph.
    async fn entity_relations(&self, entity_id: i64, limit: u32) -> Result<Vec<RelatedEntity>, StoreError>;

    /// List articles linked to an entity (evidence).
    async fn entity_articles(&self, entity_id: i64, limit: u32, offset: u32) -> Result<Vec<EntityArticle>, StoreError>;

    /// Activity summary for an entity over the last N days.
    async fn entity_activity_summary(
        &self,
        entity_id: i64,
        now: i64,
        days: i64,
    ) -> Result<EntityActivitySummary, StoreError>;
}
