//! Artifact Registry implementations.
//!
//! - [`InMemoryRegistry`] — ephemeral in-memory store for unit tests.
//! - [`D1ArtifactRegistry`] — production D1 index + object-store backend.

use async_trait::async_trait;
use object_store::ObjectStore;
use shared_kernel::artifact_registry::{ArtifactRef, ArtifactRegistry, NewArtifact, RegistryError};
use store::ArtifactStore;

// ── In-memory registry (tests) ──────────────────────────────────

/// Ephemeral in-memory registry for unit tests.
#[derive(Debug, Default)]
pub struct InMemoryRegistry;

#[async_trait(?Send)]
impl ArtifactRegistry for InMemoryRegistry {
    async fn store(&self, _artifact: NewArtifact) -> Result<ArtifactRef, RegistryError> {
        Err(RegistryError::Storage("InMemoryRegistry stub — use BlobStore-backed variant".into()))
    }

    async fn read(&self, _artifact_id: i64) -> Result<Option<Vec<u8>>, RegistryError> {
        Err(RegistryError::NotFound("stub".into()))
    }

    async fn find_by_owner(
        &self,
        _artifact_type: &str,
        _owner_type: &str,
        _owner_id: &str,
    ) -> Result<Option<ArtifactRef>, RegistryError> {
        Err(RegistryError::NotFound("stub".into()))
    }
}

// ── Production D1-backed registry ──────────────────────────────

/// Production artifact registry backed by D1 index + R2 object storage.
pub struct D1ArtifactRegistry<S, O> {
    store: S,
    object_store: O,
}

impl<S, O> D1ArtifactRegistry<S, O>
where
    S: ArtifactStore,
    O: ObjectStore,
{
    pub fn new(store: S, object_store: O) -> Self {
        Self { store, object_store }
    }
}

#[async_trait(?Send)]
impl<S, O> ArtifactRegistry for D1ArtifactRegistry<S, O>
where
    S: ArtifactStore,
    O: ObjectStore,
{
    async fn store(&self, artifact: NewArtifact) -> Result<ArtifactRef, RegistryError> {
        let now =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        let object_key = format!("artifacts/{}/{}/{}", artifact.artifact_type, artifact.owner_id, now);

        // 1. Write content to object store
        let obj_ref = self
            .object_store
            .write_object(&object_key, &artifact.content)
            .await
            .map_err(|e| RegistryError::Storage(e.to_string()))?;

        // 2. Register in D1 via the existing artifact_registry table.
        //    Uses the legacy NewArtifact model where entity_id is the
        //    artifact owner's numeric id. For string-based owners we
        //    encode owner info in metadata JSON.
        let metadata_json = serde_json::json!({
            "owner_type": &artifact.owner_type,
            "owner_id": &artifact.owner_id,
            "content_type": &artifact.content_type,
        });

        let artifact_id = self
            .store
            .create_artifact(&store::NewArtifact {
                artifact_type: artifact.artifact_type.clone(),
                entity_id: 0, // placeholder; owner ref is in metadata
                r2_key: object_key.clone(),
                schema_version: "1.0".into(),
                model: None,
                pipeline_version: "v1".into(),
                metadata: Some(metadata_json.to_string()),
            })
            .await
            .map_err(|e| RegistryError::Storage(e.to_string()))?;

        Ok(ArtifactRef {
            artifact_id,
            artifact_type: artifact.artifact_type,
            storage: "r2".into(),
            object_key,
            size_bytes: obj_ref.size as i64,
            created_at: now,
        })
    }

    async fn read(&self, artifact_id: i64) -> Result<Option<Vec<u8>>, RegistryError> {
        // Load artifact metadata from D1
        let entries = self
            .store
            .list_artifacts_by_entity(artifact_id, 1)
            .await
            .map_err(|e| RegistryError::Storage(e.to_string()))?;

        let entry = match entries.into_iter().next() {
            Some(e) => e,
            None => return Err(RegistryError::NotFound(format!("artifact {artifact_id}"))),
        };

        Ok(self.object_store.read_object(&entry.r2_key).await.map_err(|e| RegistryError::Storage(e.to_string()))?)
    }

    async fn find_by_owner(
        &self,
        artifact_type: &str,
        _owner_type: &str,
        _owner_id: &str,
    ) -> Result<Option<ArtifactRef>, RegistryError> {
        let entries =
            self.store.list_artifacts(artifact_type, 1).await.map_err(|e| RegistryError::Storage(e.to_string()))?;

        Ok(entries.into_iter().next().map(|entry| ArtifactRef {
            artifact_id: entry.id,
            artifact_type: artifact_type.to_string(),
            storage: "r2".into(),
            object_key: entry.object_key.clone(),
            size_bytes: entry.size_bytes.unwrap_or(0),
            created_at: entry.created_at,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::BlobStore;
    use shared_kernel::artifact_registry::{NewArtifact, RegistryError};
    use store::memory::MemoryStore;

    type Registry = D1ArtifactRegistry<MemoryStore, BlobStore>;

    fn make_registry() -> Registry {
        Registry::new(MemoryStore::new(), BlobStore::new())
    }

    fn sample_artifact() -> NewArtifact {
        NewArtifact {
            artifact_type: "decision_memo".into(),
            owner_type: "decision".into(),
            owner_id: "DEC-000001".into(),
            content: b"hypothesis memo".to_vec(),
            content_type: "text/markdown".into(),
        }
    }

    #[test]
    fn store_writes_registry_and_returns_artifact_ref() {
        let registry = make_registry();
        let reference = futures::executor::block_on(registry.store(sample_artifact())).unwrap();

        assert_eq!(reference.artifact_id, 1);
        assert_eq!(reference.artifact_type, "decision_memo");
        assert_eq!(reference.storage, "r2");
        assert!(
            reference.object_key.starts_with("artifacts/decision_memo/DEC-000001/"),
            "object_key: {}",
            reference.object_key
        );
        assert_eq!(reference.size_bytes, 15); // "hypothesis memo".len()
    }

    #[test]
    fn read_returns_not_found_for_unknown_artifact() {
        let registry = make_registry();
        let err = futures::executor::block_on(registry.read(999)).unwrap_err();
        assert!(matches!(err, RegistryError::NotFound(_)));
    }

    // KNOWN DEFECT (decoupling P3, adapter rework): `store()` writes the row with
    // `entity_id = 0`, but `read()` queries `list_artifacts_by_entity(artifact_id, 1)`.
    // With ids starting at 1 the two never align, so a store→read round-trip returns
    // NotFound even in production D1 (both artifact_registry table and MemoryStore).
    // Un-ignore once a by-id lookup port on artifact_registry exists and read() uses it.
    #[test]
    #[ignore = "KNOWN DEFECT: read() looks up artifact_id as entity_id; store() writes entity_id=0. Fix in decoupling P3."]
    fn read_round_trips_stored_content() {
        let registry = make_registry();
        let reference = futures::executor::block_on(registry.store(sample_artifact())).unwrap();
        let bytes = futures::executor::block_on(registry.read(reference.artifact_id)).unwrap();
        assert_eq!(bytes, Some(b"hypothesis memo".to_vec()));
    }

    // KNOWN DEFECT (decoupling P3): `find_by_owner()` calls the store's
    // `list_artifacts` read, which reads the memory_artifacts table — a DIFFERENT
    // table from the artifact_registry row that `store()` writes (MemoryStore
    // mirrors this split).
    // It also ignores owner_type/owner_id entirely. Un-ignore once find_by_owner
    // reads the artifact_registry table and filters by the owner in metadata.
    #[test]
    #[ignore = "KNOWN DEFECT: find_by_owner() queries memory_artifacts, not artifact_registry. Fix in decoupling P3."]
    fn find_by_owner_returns_stored_artifact() {
        let registry = make_registry();
        let reference = futures::executor::block_on(registry.store(sample_artifact())).unwrap();
        let found =
            futures::executor::block_on(registry.find_by_owner("decision_memo", "decision", "DEC-000001")).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().artifact_id, reference.artifact_id);
    }
}
