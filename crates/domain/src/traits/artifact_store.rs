use async_trait::async_trait;

use crate::{ArtifactEntry, ArtifactRecord, NewArtifact, StoreError};

/// Artifact-registry persistence (D1 `artifact_registry` index rows).
///
/// Lifted off [`StoreBackend`](crate::StoreBackend) in P4 so infra adapters
/// (e.g. the R2-backed artifact registry) bind this instead of the legacy
/// supertrait.  The object bytes themselves live in R2; this seam tracks the
/// index rows (type, entity, object key) used to locate and list artifacts.
#[async_trait(?Send)]
pub trait ArtifactStore {
    /// Register an R2 artifact in the artifact_registry.
    async fn create_artifact(&self, artifact: &NewArtifact) -> Result<i64, StoreError>;

    /// List artifact_registry entries for a given entity.
    async fn list_artifacts_by_entity(&self, entity_id: i64, limit: u32) -> Result<Vec<ArtifactEntry>, StoreError>;

    /// List artifacts of a given type, newest first.
    async fn list_artifacts(&self, artifact_type: &str, limit: u32) -> Result<Vec<ArtifactRecord>, StoreError>;
}
