use serde::{Deserialize, Serialize};

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
