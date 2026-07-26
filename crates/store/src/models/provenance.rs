use serde::{Deserialize, Serialize};

use crate::SourceSummary;

/// Provenance — lineage metadata carried by every intelligence artifact.
///
/// Links back to the original source observation, enabling full-chain
/// traceability: Source → Observation → Signal → Claim → Decision → Memory.
///
/// Sprint 5.6: introduced as the standard lineage contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    /// FK to the source_registry table.
    pub source_id: Option<i64>,
    /// FK to the observations table.
    pub observation_id: Option<i64>,
    /// Human-readable attribution text (e.g. "Reuters", "arXiv").
    pub attribution: Option<String>,
    /// Display name of the source.
    pub source_name: Option<String>,
    /// Canonical URL of the original content.
    pub source_url: Option<String>,
}

/// Provenance attached to an article detail response.
/// Created by service-layer composition, not SQL JOIN.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleProvenance {
    pub source: Option<SourceSummary>,
    pub attribution: Option<String>,
}

/// Full provenance chain for any intelligence entity type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceSummary {
    pub entity_type: String,
    pub entity_id: String,
    pub sources: Vec<SourceSummary>,
    pub observation_count: usize,
    pub evidence_count: usize,
    pub confidence: Option<f64>,
}
