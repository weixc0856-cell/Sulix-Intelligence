use serde::{Deserialize, Serialize};

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
