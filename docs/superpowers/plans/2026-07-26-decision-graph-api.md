# Decision Graph API — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development or executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Implement the Decision Graph MVP backend API (`GET /api/projections/decision-graph`) through GraphProjectionService.

**Architecture:** 4 tasks in dependency order: types → service → handler → route registration.

**Tech Stack:** Rust, D1, `worker::Router`

---

### Task 1: Graph types + GraphProjectionService

**Files:**
- Create: `crates/application/src/graph.rs`
- Modify: `crates/application/src/lib.rs`

**Step 1.1: Create graph.rs with all types**

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use store::StoreError;

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
pub enum GraphNodeType {
    Signal,
    Decision,
    Outcome,
    Reflection,
    Memory,
    Strategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub edge_type: GraphEdgeType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphEdgeType {
    EvidenceFor,
    Influenced,
    Triggered,
    ResultedIn,
    EvaluatedBy,
    LearnedFrom,
    AlignedWith,
}

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
    pub async fn build_decision_graph(&self, limit: u32) -> Result<GraphResponse, StoreError> {
        let now = js_sys::Date::now() as i64 / 1000;

        let decisions = self.store.list_decisions(None, limit).await?;
        if decisions.is_empty() {
            return Ok(GraphResponse::empty("decision-graph"));
        }

        let root_id = format!("DEC-{:06}", decisions[0].id);
        let signal_ids: Vec<i64> = decisions.iter().filter_map(|d| d.signal_thread_id).collect();
        let decision_ids: Vec<i64> = decisions.iter().map(|d| d.id).collect();

        // Load outcomes for all decisions
        let outcomes = if !decision_ids.is_empty() {
            // For MVP, load outcomes for the first decision to show the result
            self.store.list_outcomes(decision_ids[0]).await.ok().unwrap_or_default()
        } else {
            Vec::new()
        };

        // Build nodes
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        // Add Decision nodes
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
        }

        // Add Signal nodes + edges (from signal_thread_id FK)
        for d in &decisions {
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

        // Add Outcome nodes + edges
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
            generated_at: now,
        })
    }
}
```

**Step 1.2: Register module in lib.rs**

Add `pub mod graph;` and `pub use graph::GraphProjectionService;` to `crates/application/src/lib.rs`.

---

### Task 2: HTTP handler

**Files:**
- Create: `crates/api/src/routes/graph.rs`

```rust
use serde_json::json;
use worker::*;
use store::Store;
use crate::shared::response;

pub async fn decision_graph(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let limit: u32 = ctx.param("limit").and_then(|s| s.parse().ok()).unwrap_or(20);
    let service = application::GraphProjectionService::new(store);

    match service.build_decision_graph(limit).await {
        Ok(graph) => response::json_ok(json!(graph)),
        Err(e) => {
            console_log!("[Sulix:graph] decision_graph failed: {e}");
            response::json_err_internal("decision graph query failed")
        }
    }
}
```

---

### Task 3: Register route in Router

**Files:**
- Modify: `crates/api/src/lib.rs`

Add `mod graph;` to the module declaration section, and add:
```rust
.get_async("/api/projections/decision-graph", routes::graph::decision_graph)
```
to the router chain.

---

### Verification

```bash
cargo check --workspace
cargo test --workspace
```

Expected: API handler compiles and returns `{ projection: "decision-graph", root: {...}, nodes: [...], edges: [...], generated_at: ... }` at `GET /api/projections/decision-graph?limit=5`.
