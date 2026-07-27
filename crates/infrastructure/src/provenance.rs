//! Provenance query implementation — reads artifact_lineage from D1.

use async_trait::async_trait;
use shared_kernel::lineage_query::{LineageEntry, LineageQuery};
use store::D1Store;

/// D1-backed provenance query.
pub struct D1LineageQuery {
    store: D1Store,
}

impl D1LineageQuery {
    pub fn new(store: D1Store) -> Self {
        Self { store }
    }
}

#[async_trait(?Send)]
impl LineageQuery for D1LineageQuery {
    async fn query_lineage(
        &self,
        direction: &str,
        artifact_type: &str,
        artifact_id: &str,
    ) -> Result<Vec<LineageEntry>, String> {
        let rows = self
            .store
            .query_lineage(direction, artifact_type, artifact_id)
            .await
            .map_err(|e| format!("lineage query failed: {e}"))?;

        let entries = rows
            .into_iter()
            .map(|row| {
                let (target_type, target_id) = if direction == "from" {
                    (row.to_artifact_type, row.to_artifact_id)
                } else {
                    (row.from_artifact_type, row.from_artifact_id)
                };

                LineageEntry {
                    target_type,
                    target_id,
                    relationship: row.relationship,
                }
            })
            .collect();

        Ok(entries)
    }
}
