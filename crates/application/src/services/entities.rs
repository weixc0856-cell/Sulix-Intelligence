//! Entity Graph application service — orchestrates the Entity (Knowledge
//! Graph) read use-cases (list / get / relations / articles / activity) that
//! the API routes expose under `/api/intelligence/entities`.
//!
//! Generic over the narrowest store surface — [`EntityQueryService`].  It
//! contains zero Worker, HTTP, or `js_sys` code: the HTTP layer (`crates/api`)
//! parses requests, reads the clock via `js_sys`, and hands the timestamp in
//! as a parameter.  The 7-day activity window is an application-owned rule.

use domain::{EntityActivitySummary, EntityArticle, EntityDetail, EntitySummary, RelatedEntity, StoreError};

/// How many days of activity the [`EntityService::activity`] summary covers.
/// Business rule owned by the application layer.
const ACTIVITY_WINDOW_DAYS: u32 = 7;

/// Application service for Entity Graph read use-cases.
pub struct EntityService<S> {
    store: S,
}

impl<S> EntityService<S>
where
    S: domain::EntityQueryService,
{
    /// Wrap a store (or store-backed query service) in the service.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// List entities, ordered by article count (paginated).
    pub async fn list(&self, limit: u32, offset: u32) -> Result<Vec<EntitySummary>, StoreError> {
        self.store.list_entities(limit, offset).await
    }

    /// Get a single entity with its aggregate article count.
    pub async fn get(&self, id: i64) -> Result<Option<EntityDetail>, StoreError> {
        self.store.entity_detail(id).await
    }

    /// Get entities related to `entity_id` through graph edges.
    pub async fn relations(&self, entity_id: i64, limit: u32) -> Result<Vec<RelatedEntity>, StoreError> {
        self.store.entity_relations(entity_id, limit).await
    }

    /// List articles that evidence an entity (paginated).
    pub async fn articles(&self, entity_id: i64, limit: u32, offset: u32) -> Result<Vec<EntityArticle>, StoreError> {
        self.store.entity_articles(entity_id, limit, offset).await
    }

    /// Activity summary for an entity over the last [`ACTIVITY_WINDOW_DAYS`]
    /// days ending at `now` (unix seconds, supplied by the caller).
    pub async fn activity(&self, entity_id: i64, now: i64) -> Result<EntityActivitySummary, StoreError> {
        self.store.entity_activity_summary(entity_id, now, i64::from(ACTIVITY_WINDOW_DAYS)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::EntityRepository;
    use store::memory::MemoryStore;

    /// Seed the store with an entity, returning its id.
    fn seed_entity(store: &MemoryStore, name: &str) -> i64 {
        futures::executor::block_on(store.save_entity(name, name, "person")).expect("save_entity should succeed")
    }

    fn link_article(store: &MemoryStore, article_id: i64, entity_id: i64) {
        futures::executor::block_on(store.link_article(article_id, entity_id, 0.9)).expect("link_article succeeds");
    }

    fn link_relation(store: &MemoryStore, a: i64, b: i64) {
        futures::executor::block_on(store.link_relation(a, b, "collaborates_with")).expect("link_relation succeeds");
    }

    #[test]
    fn list_orders_by_article_count() {
        let store = MemoryStore::new();
        let quiet = seed_entity(&store, "quiet");
        let busy = seed_entity(&store, "busy");
        link_article(&store, 101, busy);
        link_article(&store, 102, busy);
        link_article(&store, 103, quiet);

        let svc = EntityService::new(store);
        let list = futures::executor::block_on(svc.list(50, 0)).expect("list should succeed");
        assert_eq!(list.len(), 2);
        // Ordered by article_count DESC → busy (2) before quiet (1).
        assert_eq!(list[0].name, "busy");
        assert_eq!(list[0].article_count, 2);
        assert_eq!(list[1].name, "quiet");
        assert_eq!(list[1].article_count, 1);
    }

    #[test]
    fn list_respects_pagination_bounds() {
        let store = MemoryStore::new();
        seed_entity(&store, "alpha");
        seed_entity(&store, "beta");

        let svc = EntityService::new(store);
        let page = futures::executor::block_on(svc.list(1, 0)).expect("list should succeed");
        assert_eq!(page.len(), 1);
        let beyond = futures::executor::block_on(svc.list(50, 99)).expect("list should succeed");
        assert!(beyond.is_empty(), "offset past the end yields no rows");
    }

    #[test]
    fn get_returns_detail_and_missing_returns_none() {
        let store = MemoryStore::new();
        let id = seed_entity(&store, "openai");
        link_article(&store, 7, id);

        let svc = EntityService::new(store);
        let detail =
            futures::executor::block_on(svc.get(id)).expect("get should succeed").expect("entity should exist");
        assert_eq!(detail.name, "openai");
        assert_eq!(detail.entity_type, "person");
        assert_eq!(detail.article_count, 1);

        assert!(futures::executor::block_on(svc.get(999)).expect("get should succeed").is_none());
    }

    #[test]
    fn relations_returns_linked_entities() {
        let store = MemoryStore::new();
        let a = seed_entity(&store, "a");
        let b = seed_entity(&store, "b");
        let c = seed_entity(&store, "c");
        link_relation(&store, a, b);
        link_relation(&store, a, c);

        let svc = EntityService::new(store);
        let relations = futures::executor::block_on(svc.relations(a, 50)).expect("relations should succeed");
        assert_eq!(relations.len(), 2);
        let names: Vec<String> = relations.iter().map(|r| r.name.clone()).collect();
        assert!(names.contains(&"b".into()));
        assert!(names.contains(&"c".into()));
        assert!(relations.iter().all(|r| r.relation_type == "collaborates_with"));
    }

    #[test]
    fn articles_returns_linked_evidence_paginated() {
        let store = MemoryStore::new();
        let id = seed_entity(&store, "nvidia");
        link_article(&store, 101, id);
        link_article(&store, 102, id);

        let svc = EntityService::new(store);
        // MemoryStore lists article ids descending; ask for one at a time.
        let first = futures::executor::block_on(svc.articles(id, 1, 0)).expect("articles should succeed");
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].id, 102);
        let second = futures::executor::block_on(svc.articles(id, 1, 1)).expect("articles should succeed");
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].id, 101);
    }

    #[test]
    fn activity_uses_fixed_now_and_counts_evidence() {
        let store = MemoryStore::new();
        let busy = seed_entity(&store, "busy");
        let lonely = seed_entity(&store, "lonely");
        link_article(&store, 101, busy);
        link_article(&store, 102, busy);
        link_article(&store, 102, busy); // duplicate link — still one article

        let svc = EntityService::new(store);
        // Fixed `now` — no wall-clock dependency in the test or the service.
        let now = 1_700_000_000;
        let summary = futures::executor::block_on(svc.activity(busy, now)).expect("activity should succeed");
        assert_eq!(summary.article_count, 2, "distinct evidence articles within the window");
        let none = futures::executor::block_on(svc.activity(lonely, now)).expect("activity should succeed");
        assert_eq!(none.article_count, 0);
    }
}
