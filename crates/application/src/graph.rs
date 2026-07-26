//! Decision Graph Projection — Generic Projection Contract v1.
//!
//! Returns a render-ready node+edge structure so the frontend can
//! visualize a decision's Signal evidence → Decision hypothesis → Outcome.
//!
//! MVP scope: Signal → Decision → Outcome (Reflection/Memory/Strategy deferred).
//! Not a graph database — a read-model Projection.

use serde::{Deserialize, Serialize};
use store::StoreError;

// ── Graph Response (Generic Projection Contract) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphResponse {
    pub projection: String,
    pub root: GraphRoot,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub generated_at: i64,
}

impl GraphResponse {
    pub fn empty(projection: &str) -> Self {
        Self {
            projection: projection.into(),
            root: GraphRoot { id: String::new(), node_type: GraphNodeType::Decision },
            nodes: Vec::new(),
            edges: Vec::new(),
            generated_at: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRoot {
    pub id: String,
    pub node_type: GraphNodeType,
}

// ── Node ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub entity_id: i64,
    pub node_type: GraphNodeType,
    pub title: String,
    pub data: GraphNodeData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNodeData {
    pub confidence: Option<f64>,
    pub status: Option<String>,
    pub source_count: Option<u32>,
    pub article_count: Option<u32>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphNodeType {
    Signal,
    Decision,
    Outcome,
    Reflection,
    Memory,
    Strategy,
}

// ── Edge ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    #[serde(rename = "edge_type")]
    pub edge_type: GraphEdgeType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphEdgeType {
    EvidenceFor,
    Influenced,
    Triggered,
    ResultedIn,
    EvaluatedBy,
    LearnedFrom,
    AlignedWith,
}

// ── Projection Service ──

pub struct GraphProjectionService<S> {
    store: S,
}

impl<S> GraphProjectionService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

impl<S> GraphProjectionService<S>
where
    S: store::DecisionQueryService + store::OutcomeQueryService,
{
    /// Build the decision-centric graph projection.
    ///
    /// 1. Load recent decisions (1 query)
    /// 2. Extract signal_thread_ids + decision_ids
    /// 3. Load outcomes for decisions (1 query)
    /// 4. Assemble nodes + edges → GraphResponse
    pub async fn build_decision_graph(&self, limit: u32) -> Result<GraphResponse, StoreError> {
        let decisions = self.store.list_decisions(None, limit).await?;
        if decisions.is_empty() {
            return Ok(GraphResponse::empty("decision-graph"));
        }

        let root_id = format!("DEC-{:06}", decisions[0].id);
        let decision_ids: Vec<i64> = decisions.iter().map(|d| d.id).collect();

        // For MVP, load outcomes for the first decision
        let outcomes = <S as store::OutcomeQueryService>::list_outcomes(
            &self.store,
            decision_ids[0],
        )
        .await
        .unwrap_or_default();

        let mut nodes: Vec<GraphNode> = Vec::new();
        let mut edges: Vec<GraphEdge> = Vec::new();

        // Decision nodes
        for d in &decisions {
            nodes.push(GraphNode {
                id: format!("DEC-{:06}", d.id),
                entity_id: d.id,
                node_type: GraphNodeType::Decision,
                title: d.title.clone(),
                data: GraphNodeData {
                    confidence: Some(d.confidence),
                    status: Some(d.status.clone()),
                    source_count: None,
                    article_count: None,
                    url: Some(format!("/intelligence/decisions/{}", d.id)),
                },
            });

            // Signal → Decision edges (from signal_thread_id FK)
            if let Some(sid) = d.signal_thread_id {
                nodes.push(GraphNode {
                    id: format!("SIG-{:06}", sid),
                    entity_id: sid,
                    node_type: GraphNodeType::Signal,
                    title: format!("Signal #{}", sid),
                    data: GraphNodeData {
                        confidence: None,
                        status: None,
                        source_count: None,
                        article_count: None,
                        url: Some(format!("/intelligence/signals/{}", sid)),
                    },
                });
                edges.push(GraphEdge {
                    source: format!("SIG-{:06}", sid),
                    target: format!("DEC-{:06}", d.id),
                    edge_type: GraphEdgeType::Influenced,
                });
            }
        }

        // Outcome nodes + edges
        for o in &outcomes {
            let out_id = format!("OUT-{:06}", o.id);
            nodes.push(GraphNode {
                id: out_id.clone(),
                entity_id: o.id,
                node_type: GraphNodeType::Outcome,
                title: o.outcome_type.clone(),
                data: GraphNodeData {
                    confidence: None,
                    status: Some(o.observation.clone()),
                    source_count: None,
                    article_count: None,
                    url: None,
                },
            });
            edges.push(GraphEdge {
                source: root_id.clone(),
                target: out_id,
                edge_type: GraphEdgeType::ResultedIn,
            });
        }

        Ok(GraphResponse {
            projection: "decision-graph".into(),
            root: GraphRoot { id: root_id, node_type: GraphNodeType::Decision },
            nodes,
            edges,
            generated_at: 0, // Timestamp injected by caller in production
        })
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use store::memory::MemoryStore;

    #[test]
    fn test_empty_store_returns_empty_graph() {
        let store = MemoryStore::new();
        let service = GraphProjectionService::new(store);
        let result = futures::executor::block_on(service.build_decision_graph(10)).unwrap();
        assert_eq!(result.projection, "decision-graph");
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
    }
}
