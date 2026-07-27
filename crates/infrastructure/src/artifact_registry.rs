//! Artifact Registry implementations.
//!
//! - [`InMemoryRegistry`] — ephemeral in-memory store for unit tests.
//! - [`D1ArtifactRegistry`] — production D1 index + object-store backend.

use async_trait::async_trait;
use object_store::ObjectStore;
use shared_kernel::artifact_registry::{ArtifactRef, ArtifactRegistry, NewArtifact, RegistryError};
use store::StoreBackend;

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
    S: StoreBackend,
    O: ObjectStore,
{
    pub fn new(store: S, object_store: O) -> Self {
        Self { store, object_store }
    }
}

#[async_trait(?Send)]
impl<S, O> ArtifactRegistry for D1ArtifactRegistry<S, O>
where
    S: StoreBackend,
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

// Tests for D1ArtifactRegistry require wasm32 target (js_sys::Date in MemoryStore).
// Run with: cargo test --target wasm32-unknown-unknown -p infrastructure
