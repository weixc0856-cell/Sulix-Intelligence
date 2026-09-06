//! Provenance query implementation — reads artifact_lineage from D1.

use async_trait::async_trait;
use shared_kernel::lineage_query::{LineageEntry, LineageQuery};
use store::d1::provenance::LineageRow;
use store::D1Store;

/// Which side of a lineage edge is the query target.
///
/// - `direction == "from"`: the row records `(query_artifact → other)`, so the
///   target is the `to` side (the query artifact's children).
/// - otherwise: the row records `(other → query_artifact)`, so the target is the
///   `from` side (the query artifact's parents).
///
/// Extracted from the adapter loop as a pure function so the direction semantics
/// are unit-testable without a D1 connection (`query_lineage` is a direct
/// `impl D1Store` method, so the adapter itself is not host-injectable until
/// decoupling P3 turns this into a real port).
fn target_side<'a>(direction: &str, row: &'a LineageRow) -> (&'a str, &'a str) {
    if direction == "from" {
        (&row.to_artifact_type, &row.to_artifact_id)
    } else {
        (&row.from_artifact_type, &row.from_artifact_id)
    }
}

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
                let (target_type, target_id) = target_side(direction, &row);
                LineageEntry {
                    target_type: target_type.to_string(),
                    target_id: target_id.to_string(),
                    relationship: row.relationship,
                }
            })
            .collect();

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> LineageRow {
        LineageRow {
            from_artifact_type: "briefing".into(),
            from_artifact_id: "B-1".into(),
            to_artifact_type: "decision".into(),
            to_artifact_id: "DEC-1".into(),
            relationship: "derived_from".into(),
        }
    }

    #[test]
    fn from_direction_resolves_to_children() {
        let r = row();
        let (t, id) = target_side("from", &r);
        assert_eq!((t, id), ("decision", "DEC-1"));
    }

    #[test]
    fn to_direction_resolves_to_parents() {
        let r = row();
        let (t, id) = target_side("to", &r);
        assert_eq!((t, id), ("briefing", "B-1"));
    }

    #[test]
    fn unknown_direction_defaults_to_parent_side() {
        let r = row();
        let (t, id) = target_side("sideways", &r);
        assert_eq!((t, id), ("briefing", "B-1"));
    }
}
