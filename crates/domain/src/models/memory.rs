use serde::{Deserialize, Serialize};

/// A row from the memory_index table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: i64,
    pub memory_type: String,
    pub memory_origin: String,
    pub statement: String,
    pub confidence: f64,
    pub stability_score: Option<f64>,
    pub confidence_updated_at: Option<i64>,
    pub memory_sources: Option<String>, // JSON array, deserialized to Vec<MemorySourceRef>
    pub artifact_key: Option<String>,
    pub status: String,
    pub usage_count: i64,
    pub validation_count: i64,
    pub promoted_at: i64,
    pub deprecated_at: Option<i64>,
    pub last_used_at: Option<i64>,
    pub created_at: i64,
}

/// Input for inserting a new memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMemory {
    pub memory_type: String,
    pub memory_origin: String,
    pub statement: String,
    pub confidence: f64,
    pub stability_score: Option<f64>,
    pub memory_sources: Option<String>,
    pub artifact_key: Option<String>,
    pub status: String,
}

/// A single source reference in the memory lineage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySourceRef {
    pub source_type: String,
    pub source_id: String,
}

/// Promotion score — calculated in the MemoryEvaluator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromotionScore {
    pub confidence: f32,
    pub recurrence: f32,
    pub impact: f32,
    pub evidence: f32,
    pub stability: f32,
    pub total: f32,
}
