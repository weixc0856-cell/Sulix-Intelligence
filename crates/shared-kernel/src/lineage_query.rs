//! LineageQuery trait — resolves artifact lineage from the provenance store.
//!
//! Implementations live in infrastructure (D1) and test (memory).

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lineage_entry_is_a_plain_dto_with_named_fields() {
        let e = LineageEntry {
            target_type: "decision".into(),
            target_id: "DEC-000001".into(),
            relationship: "derived_from".into(),
        };
        assert_eq!(e.target_type, "decision");
        assert_eq!(e.target_id, "DEC-000001");
        assert_eq!(e.relationship, "derived_from");
    }

    struct FakeLineage(Vec<LineageEntry>);

    #[async_trait(?Send)]
    impl LineageQuery for FakeLineage {
        async fn query_lineage(&self, _d: &str, _t: &str, _id: &str) -> Result<Vec<LineageEntry>, String> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn trait_contract_is_drivable_with_block_on() {
        let fake = FakeLineage(vec![LineageEntry {
            target_type: "briefing".into(),
            target_id: "B-1".into(),
            relationship: "derived_from".into(),
        }]);
        let out = futures::executor::block_on(fake.query_lineage("from", "decision", "DEC-1")).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].target_type, "briefing");
        assert_eq!(out[0].target_id, "B-1");
    }
}
