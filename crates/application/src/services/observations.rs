//! Observation application service — read-only access to structured observation
//! records and their lineage chain (Source → Observation).
//!
//! Generic over the narrowest store surface — [`domain::ObservationQueryService`]
//! for listing, [`domain::ObservationRepository`] for row lookup and
//! [`domain::SourceRepository`] for registry-source resolution.

use domain::{Observation, Source, StoreError};

/// Application service for Observation read use-cases.
pub struct ObservationService<S> {
    store: S,
}

impl<S> ObservationService<S>
where
    S: domain::ObservationQueryService + domain::ObservationRepository + domain::SourceRepository,
{
    /// Wrap a store (or store-backed repository/query pair) in the service.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// List observations, optionally filtered by source.
    pub async fn list(
        &self,
        source_type: Option<&str>,
        source_id: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Observation>, StoreError> {
        self.store.list_observations(source_type, source_id, limit, offset).await
    }

    /// Get a single observation by its primary key.
    pub async fn get(&self, id: i64) -> Result<Option<Observation>, StoreError> {
        self.store.find_observation(id).await
    }

    /// Load an observation together with its registry-source metadata, if one
    /// is linked.
    ///
    /// Source resolution is enrichment: a lookup failure degrades to `None` for
    /// the source (matches the historical handler behaviour, which used
    /// `ok().flatten()`).
    pub async fn lineage(&self, id: i64) -> Result<Option<(Observation, Option<Source>)>, StoreError> {
        let observation = match self.store.find_observation(id).await? {
            Some(o) => o,
            None => return Ok(None),
        };
        let source = match observation.registry_source_id {
            Some(sid) => self.store.find_source(sid).await.ok().flatten(),
            None => None,
        };
        Ok(Some((observation, source)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use store::memory::MemoryStore;

    // MemoryStore's observation backend is intentionally a stub (save is a
    // no-op, lookups return `None`), so these tests pin the service contract
    // for the paths the stub can express — missing rows must surface as
    // `Ok(None)`, never as an error.  No MemoryStore behaviour is expanded here.

    #[test]
    fn get_missing_returns_none() {
        let svc = ObservationService::new(MemoryStore::new());
        assert!(futures::executor::block_on(svc.get(42)).expect("get should succeed").is_none());
    }

    #[test]
    fn lineage_missing_returns_none() {
        let svc = ObservationService::new(MemoryStore::new());
        assert!(futures::executor::block_on(svc.lineage(42)).expect("lineage should succeed").is_none());
    }

    #[test]
    fn list_returns_empty_from_stub_backend() {
        let svc = ObservationService::new(MemoryStore::new());
        let rows = futures::executor::block_on(svc.list(None, None, 50, 0)).expect("list should succeed");
        assert!(rows.is_empty());
    }
}
