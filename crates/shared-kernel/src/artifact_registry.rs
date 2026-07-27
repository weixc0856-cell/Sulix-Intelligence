//! Artifact Registry — infrastructure contract for large-object storage.
//!
//! Domain entities reference artifacts by `artifact_id` instead of raw R2/S3
//! keys, so storage backend changes (R2 → S3 → IPFS) never leak into
//! business logic.
//!
//! See the companion infrastructure implementations:
//! - `crates/infrastructure/src/artifact_registry.rs` — InMemory + D1-backed

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A reference to a stored artifact.
///
/// Returned by [`ArtifactRegistry::store`] and used by domain entities
/// to point at their large-object data without knowing the storage backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRef {
    /// Primary key in the `artifacts` registry table.
    pub artifact_id: i64,
    /// Logical type, e.g. `"decision_memo"`, `"reflection_result"`, `"reasoning_trace"`.
    pub artifact_type: String,
    /// Storage backend, e.g. `"r2"`, `"s3"`, `"ipfs"`.
    pub storage: String,
    /// Backend-native key (e.g. R2 object key).
    pub object_key: String,
    /// Content size in bytes.
    pub size_bytes: i64,
    /// When the artifact was created (unix timestamp).
    pub created_at: i64,
}

/// Input for storing a new artifact.
#[derive(Debug, Clone)]
pub struct NewArtifact {
    /// Logical type (`"decision_memo"`, `"reflection_result"`, etc.).
    pub artifact_type: String,
    /// Domain entity type that owns this artifact (`"decision"`, `"reflection"`).
    pub owner_type: String,
    /// Domain entity ID (`"DEC-000001"`, `"REF-000001"`).
    pub owner_id: String,
    /// Raw content bytes.
    pub content: Vec<u8>,
    /// MIME type (`"application/json"`, `"text/markdown"`).
    pub content_type: String,
}

/// Registry for storing and retrieving AI-generated artifacts.
///
/// This is the **canonical** path for all large objects. Domain entities
/// reference artifacts by `artifact_id` — never by raw R2 keys.
///
/// ## Implementations
///
/// | Implementation | Location | Purpose |
/// |----------------|----------|---------|
/// | `InMemoryRegistry` | `crates/infrastructure/` | Unit tests |
/// | `D1ArtifactRegistry` | `crates/infrastructure/d1/` | Production (D1 index + object-store) |
#[async_trait(?Send)]
pub trait ArtifactRegistry {
    /// Store a new artifact and return a reference.
    async fn store(&self, artifact: NewArtifact) -> Result<ArtifactRef, RegistryError>;

    /// Retrieve raw content by artifact ID.
    async fn read(&self, artifact_id: i64) -> Result<Option<Vec<u8>>, RegistryError>;

    /// Find an artifact reference by type + owner.
    async fn find_by_owner(
        &self,
        artifact_type: &str,
        owner_type: &str,
        owner_id: &str,
    ) -> Result<Option<ArtifactRef>, RegistryError>;
}

/// Errors from ArtifactRegistry operations.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("storage backend error: {0}")]
    Storage(String),
    #[error("artifact not found: {0}")]
    NotFound(String),
    #[error("serialisation error: {0}")]
    Serialisation(String),
}
