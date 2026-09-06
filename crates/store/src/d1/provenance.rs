//! D1 provenance queries — resolves artifact lineage from D1.
//!
//! Direct `impl D1Store` method.

use crate::s_err::StoreResultExt;
use serde::Deserialize;

use crate::StoreError;

/// A row from the artifact_lineage table.
#[derive(Debug, Clone, Deserialize)]
pub struct LineageRow {
    pub from_artifact_type: String,
    pub from_artifact_id: String,
    pub to_artifact_type: String,
    pub to_artifact_id: String,
    pub relationship: String,
}

impl crate::D1Store {
    /// Query artifact_lineage for all rows matching a given direction.
    ///
    /// `direction` = "from" (this → others, children) or "to" (others → this, parents).
    pub async fn query_lineage(
        &self,
        direction: &str,
        artifact_type: &str,
        artifact_id: &str,
    ) -> Result<Vec<LineageRow>, StoreError> {
        let (select_clause, type_col, id_col) = if direction == "from" {
            (
                "from_artifact_type, from_artifact_id, to_artifact_type, to_artifact_id, relationship",
                "from_artifact_type",
                "from_artifact_id",
            )
        } else {
            (
                "from_artifact_type, from_artifact_id, to_artifact_type, to_artifact_id, relationship",
                "to_artifact_type",
                "to_artifact_id",
            )
        };

        let sql = format!(
            "SELECT {} FROM artifact_lineage WHERE {} = ?1 AND {} = ?2 ORDER BY id",
            select_clause, type_col, id_col,
        );

        let rows = self
            .db
            .prepare(&sql)
            .bind(&[artifact_type.into(), artifact_id.into()])
            .s_err()?
            .all()
            .await
            .s_err()?
            .results::<LineageRow>()
            .s_err()?;

        Ok(rows)
    }
}
