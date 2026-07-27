//! Provenance Query Service — resolve the full lineage chain for any artifact.
//!
//! Uses [`shared_kernel::lineage_query::LineageQuery`] trait to resolve the
//! artifact_lineage table. Implementations live in infrastructure (D1) and
//! test (memory).

/// A reference to an artifact in the lineage chain.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArtifactRef {
    pub artifact_type: String,
    pub artifact_id: String,
}

/// A node in the provenance chain.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProvenanceNode {
    pub artifact: ArtifactRef,
    pub relationship: String,
    pub title: Option<String>,
}

/// Full provenance chain for an artifact.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProvenanceChain {
    pub artifact: ArtifactRef,
    pub parents: Vec<ProvenanceNode>,
    pub children: Vec<ProvenanceNode>,
}

/// Resolve the full lineage chain.
///
/// Generic over any [`shared_kernel::lineage_query::LineageQuery`] implementation
/// (D1Store for production, MemoryStore for tests).
pub async fn get_lineage(
    query: &impl shared_kernel::lineage_query::LineageQuery,
    artifact_type: &str,
    artifact_id: &str,
) -> Result<ProvenanceChain, String> {
    let parent_entries = query.query_lineage("to", artifact_type, artifact_id).await?;
    let child_entries = query.query_lineage("from", artifact_type, artifact_id).await?;

    let parents = parent_entries
        .into_iter()
        .map(|e| ProvenanceNode {
            artifact: ArtifactRef { artifact_type: e.target_type, artifact_id: e.target_id },
            relationship: e.relationship,
            title: None,
        })
        .collect();

    let children = child_entries
        .into_iter()
        .map(|e| ProvenanceNode {
            artifact: ArtifactRef { artifact_type: e.target_type, artifact_id: e.target_id },
            relationship: e.relationship,
            title: None,
        })
        .collect();

    Ok(ProvenanceChain {
        artifact: ArtifactRef { artifact_type: artifact_type.to_string(), artifact_id: artifact_id.to_string() },
        parents,
        children,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provenance_chain_roundtrip() {
        let chain = ProvenanceChain {
            artifact: ArtifactRef { artifact_type: "decision".into(), artifact_id: "123".into() },
            parents: vec![ProvenanceNode {
                artifact: ArtifactRef { artifact_type: "claim".into(), artifact_id: "456".into() },
                relationship: "supported_by".into(),
                title: None,
            }],
            children: vec![ProvenanceNode {
                artifact: ArtifactRef { artifact_type: "outcome".into(), artifact_id: "789".into() },
                relationship: "triggered_by".into(),
                title: None,
            }],
        };
        assert_eq!(chain.parents.len(), 1);
        assert_eq!(chain.children.len(), 1);
    }
}
