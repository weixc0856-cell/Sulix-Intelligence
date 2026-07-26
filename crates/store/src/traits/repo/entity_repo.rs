use async_trait::async_trait;

use crate::{EntityDetail, StoreError};

/// Entity (Knowledge Graph) persistence.
///
/// Manages the named-entity registry and article–entity / entity–entity links.
/// Complex entity queries (relations, signal candidates, activity) belong in
/// [`super::super::query::EntityQueryService`].
#[async_trait(?Send)]
pub trait EntityRepository {
    /// Upsert an entity by normalized_name.  Returns the entity id.
    async fn save_entity(&self, name: &str, normalized_name: &str, entity_type: &str) -> Result<i64, StoreError>;

    /// Load an entity by its primary key.
    async fn find_entity(&self, id: i64) -> Result<Option<EntityDetail>, StoreError>;

    /// Link an article to an entity (many-to-many).
    async fn link_article(&self, article_id: i64, entity_id: i64, relevance: f64) -> Result<(), StoreError>;

    /// Link two entities with a directed relation.
    async fn link_relation(&self, source: i64, target: i64, rtype: &str) -> Result<(), StoreError>;
}
