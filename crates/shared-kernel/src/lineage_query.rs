//! LineageQuery trait — resolves artifact lineage from the provenance store.
//!
//! Implementations live in infrastructure (D1) and test (memory).
//! This is NOT on StoreBackend (which is frozen).

use async_trait::async_trait;

/// A single row from the artifact_lineage query.
///
/// Returned by [`LineageQuery::query_lineage`]. The meaning of
/// `target_type`/`target_id` depends on the query direction:
///
/// - `"from"` (this → others, children): target = `to_*` columns
/// - `"to"` (others → this, parents): target = `from_*` columns
#[derive(Debug, Clone)]
pub struct LineageEntry {
    pub target_type: String,
    pub target_id: String,
    pub relationship: String,
}

/// Queries the artifact_lineage table for parent/child relationships.
#[async_trait(?Send)]
pub trait LineageQuery {
    /// Find all lineage entries in a given direction.
    async fn query_lineage(
        &self,
        direction: &str,
        artifact_type: &str,
        artifact_id: &str,
    ) -> Result<Vec<LineageEntry>, String>;
}
