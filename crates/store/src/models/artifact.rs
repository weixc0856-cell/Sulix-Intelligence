use serde::{Deserialize, Serialize};

// ── Legacy artifact_registry (article_snapshot, Sprint 4.x) ──

/// Input for creating a new artifact_registry entry.
#[derive(Debug, Clone)]
pub struct NewArtifact {
    pub artifact_type: String,
    pub entity_id: i64,
    pub r2_key: String,
    pub schema_version: String,
    pub model: Option<String>,
    pub pipeline_version: String,
    pub metadata: Option<String>,
}

/// Entry in the artifact_registry — unified metadata for all R2-stored assets.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ArtifactEntry {
    pub id: i64,
    pub artifact_type: String,
    pub entity_id: i64,
    pub r2_key: String,
    pub schema_version: String,
    pub model: Option<String>,
    pub pipeline_version: String,
    pub metadata: Option<String>,
    pub created_at: i64,
}

// ── Memory Artifacts (Sprint 5.1+, unified Memory Archive index) ──

/// Input for registering a new artifact in the `memory_artifacts` index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewArtifactRecord {
    pub artifact_type: String,
    pub artifact_date: String,
    pub object_key: String,
    pub schema_version: i32,
    pub content_hash: Option<String>,
    pub size_bytes: Option<i64>,
    pub metadata: Option<String>,
}

/// A row in the `memory_artifacts` table — metadata index for R2 Memory Archive objects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub id: i64,
    pub artifact_type: String,
    pub artifact_date: String,
    pub object_key: String,
    pub schema_version: i32,
    pub content_hash: Option<String>,
    pub size_bytes: Option<i64>,
    pub metadata: Option<String>,
    pub created_at: i64,
}
