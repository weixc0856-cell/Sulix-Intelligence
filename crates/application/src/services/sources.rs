//! Source Registry application service — orchestrates the Source governance
//! use-cases (list / get / create / update / delete) that the API routes
//! expose under `/api/sources`.
//!
//! Generic over the narrowest store surface — [`SourceQueryService`] for
//! reads and [`SourceRepository`] for writes.  It contains zero Worker, HTTP,
//! or `js_sys` code; the HTTP layer (`crates/api`) parses requests and builds
//! the `Store`, then delegates the orchestration here.

use store::{NewSource, Source, StoreError};

/// Application service for the Source Registry use-cases.
pub struct SourceService<S> {
    store: S,
}

impl<S> SourceService<S>
where
    S: store::SourceQueryService + store::SourceRepository,
{
    /// Wrap a store (or store-backed repository/query pair) in the service.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// List sources, optionally filtered by governance tier / usage policy.
    pub async fn list(
        &self,
        tier: Option<&str>,
        policy: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Source>, StoreError> {
        self.store.list_sources(tier, policy, limit, offset).await
    }

    /// Get a single source by its primary key.
    pub async fn get(&self, id: i64) -> Result<Option<Source>, StoreError> {
        self.store.find_source(id).await
    }

    /// Register a new source.
    pub async fn create(&self, source: &NewSource) -> Result<i64, StoreError> {
        self.store.save_source(source).await
    }

    /// Update a source, preserving its `feed_id` link when the request body
    /// omits one.
    ///
    /// The write path upserts on `feed_id` (`ON CONFLICT(feed_id)`), so a
    /// dropped `feed_id` would silently insert an orphan row instead of
    /// updating the target source.  Preserving the existing link is therefore
    /// an application invariant, not a handler concern.
    pub async fn update(&self, id: i64, source: &NewSource) -> Result<i64, StoreError> {
        let mut update = source.clone();
        if update.feed_id.is_none() {
            // Look up the existing link.  On lookup failure the body is passed
            // through unchanged (matches the historical handler behaviour).
            if let Ok(Some(existing)) = self.store.find_source(id).await {
                update.feed_id = existing.feed_id;
            }
        }
        self.store.save_source(&update).await
    }

    /// Remove a source by its primary key.
    pub async fn delete(&self, id: i64) -> Result<(), StoreError> {
        self.store.delete_source(id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use store::memory::MemoryStore;

    fn new_source(feed_id: Option<i64>, name: &str) -> NewSource {
        NewSource {
            source_type: "rss".into(),
            feed_id,
            name: Some(name.into()),
            tier: "tier_0".into(),
            policy: "full_text_allowed".into(),
            license: "public_domain".into(),
            license_detail: None,
            attribution: None,
            trust_score: None,
            retention_days: None,
            verified: false,
            notes: None,
        }
    }

    fn service() -> SourceService<MemoryStore> {
        SourceService::new(MemoryStore::new())
    }

    #[test]
    fn create_then_get_roundtrips_fields() {
        let svc = service();
        let id = futures::executor::block_on(svc.create(&new_source(Some(7), "acme"))).expect("create should succeed");

        let stored =
            futures::executor::block_on(svc.get(id)).expect("get should succeed").expect("source should exist");
        assert_eq!(stored.id, id);
        assert_eq!(stored.name.as_deref(), Some("acme"));
        assert_eq!(stored.feed_id, Some(7));
        assert_eq!(stored.tier, "tier_0");
    }

    #[test]
    fn get_missing_returns_none() {
        let svc = service();
        assert!(futures::executor::block_on(svc.get(999)).expect("get should succeed").is_none());
    }

    #[test]
    fn list_returns_sources_and_respects_filters() {
        let svc = service();
        futures::executor::block_on(svc.create(&new_source(Some(1), "alpha"))).unwrap();
        futures::executor::block_on(svc.create(&new_source(Some(2), "beta"))).unwrap();

        // No filters → all sources.
        let all = futures::executor::block_on(svc.list(None, None, 50, 0)).expect("list should succeed");
        assert_eq!(all.len(), 2);
        let names: Vec<Option<String>> = all.iter().map(|s| s.name.clone()).collect();
        assert!(names.contains(&Some("alpha".into())));
        assert!(names.contains(&Some("beta".into())));

        // Filter by tier → still both (both are tier_0).
        let tiered = futures::executor::block_on(svc.list(Some("tier_0"), None, 50, 0)).unwrap();
        assert_eq!(tiered.len(), 2);

        // Filter by a policy nothing matches → empty.
        let none = futures::executor::block_on(svc.list(Some("tier_0"), Some("metadata_only"), 50, 0)).unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn update_without_feed_id_preserves_existing_feed_id() {
        let svc = service();
        let id =
            futures::executor::block_on(svc.create(&new_source(Some(7), "original"))).expect("create should succeed");

        // Update with the feed_id omitted → the existing link must be kept.
        let mut renamed = new_source(None, "renamed");
        renamed.verified = true;
        let updated_id = futures::executor::block_on(svc.update(id, &renamed)).expect("update should succeed");

        // Final state: the saved payload carries the preserved feed_id (this
        // is what lets the D1 upsert match the original row) together with
        // the renamed fields.
        let updated = futures::executor::block_on(svc.get(updated_id))
            .expect("get should succeed")
            .expect("updated source should exist");
        assert_eq!(updated.feed_id, Some(7), "omitted feed_id must be preserved from the existing source");
        assert_eq!(updated.name.as_deref(), Some("renamed"));
        assert!(updated.verified);
    }

    #[test]
    fn update_with_provided_feed_id_uses_new_value() {
        let svc = service();
        let id = futures::executor::block_on(svc.create(&new_source(Some(7), "original"))).unwrap();

        // Update supplying a fresh feed_id → the new value must flow through.
        let retargeted = new_source(Some(99), "retargeted");
        let new_id = futures::executor::block_on(svc.update(id, &retargeted)).expect("update should succeed");
        assert_ne!(new_id, id, "MemoryStore save appends a new row on upsert-miss");

        let stored =
            futures::executor::block_on(svc.get(new_id)).expect("get should succeed").expect("new row should exist");
        assert_eq!(stored.feed_id, Some(99), "provided feed_id must reach storage");
        assert_eq!(stored.name.as_deref(), Some("retargeted"));
    }

    #[test]
    fn delete_removes_source() {
        let svc = service();
        let id = futures::executor::block_on(svc.create(&new_source(Some(7), "doomed"))).unwrap();

        futures::executor::block_on(svc.delete(id)).expect("delete should succeed");
        assert!(futures::executor::block_on(svc.get(id)).expect("get should succeed").is_none());
    }
}
