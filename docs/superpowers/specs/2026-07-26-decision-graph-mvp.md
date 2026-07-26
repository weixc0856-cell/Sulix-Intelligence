# Decision Graph MVP — Design Spec

> Graph Projection Contract v1 for the Decision Intelligence Graph.
> MVP 范围：Signal → Decision → Outcome 链路（不包含 Reflection / Memory / Strategy）。
> 核心原则：以 Decision 为中心，回答"我的判断有没有被验证"。

**Goal:** Build a dedicated Graph Projection endpoint returning a render-ready node+edge structure, so the frontend can visualize a decision's Signal evidence → Decision hypothesis → Outcome verification.

**Architecture:** Projection layer in `application/` crate reads from existing D1 tables via batch queries, maps to `GraphResponse`, served at `GET /api/projections/decision-graph`. No new D1 migration needed.

**Tech Stack:** Rust + Cloudflare Workers + D1 + Astro (React Flow based SVG canvas).

---

## Core Data Structures

```rust
// === GraphResponse — top-level Generic Projection Contract ===

pub struct GraphResponse {
    pub projection: String,           // "decision-graph"
    pub root: GraphRoot,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub generated_at: i64,
}

pub struct GraphRoot {
    pub id: String,
    pub node_type: GraphNodeType,
}

// === GraphNode — a single entity in the graph ===

pub struct GraphNode {
    pub id: String,                   // Display ID, e.g. "DEC-000123"
    pub entity_id: i64,               // Domain primary key
    pub node_type: GraphNodeType,
    pub title: String,
    pub data: GraphNodeData,
}

pub struct GraphNodeData {
    pub confidence: Option<f64>,      // Decision confidence / Signal health_score
    pub status: Option<String>,
    pub source_count: Option<u32>,
    pub article_count: Option<u32>,
    pub url: Option<String>,          // Frontend drill-down route
}

pub enum GraphNodeType {
    Signal,
    Decision,
    Outcome,
    Reflection,
    Memory,
    Strategy,
}

// === GraphEdge — a directed relation between two nodes ===

pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub edge_type: GraphEdgeType,
}

pub enum GraphEdgeType {
    EvidenceFor,      // Observation / Article → Signal
    Influenced,       // Signal → Decision
    Triggered,        // Signal/Event → Decision
    ResultedIn,       // Decision → Outcome
    EvaluatedBy,      // Outcome → Reflection
    LearnedFrom,      // Reflection → Memory
    AlignedWith,      // Decision / Memory → Strategy
}
```

## API Endpoint

### GET /api/projections/decision-graph

Returns the N most recent decisions with their associated signals and outcomes.

**Parameters:**
- `?limit=20` (default 20)

**Response:**

```json
{
  "projection": "decision-graph",
  "root": { "id": "DEC-000123", "node_type": "decision" },
  "nodes": [
    {
      "id": "DEC-000123",
      "entity_id": 123,
      "node_type": "decision",
      "title": "AI Agent Investment",
      "data": {
        "confidence": 0.82,
        "status": "active",
        "source_count": 12,
        "article_count": 47,
        "url": "/intelligence/decisions/123"
      }
    },
    {
      "id": "SIG-000042",
      "entity_id": 42,
      "node_type": "signal",
      "title": "Open Source Agent Framework Explosion",
      "data": {
        "confidence": 0.75,
        "status": "active",
        "source_count": 8,
        "article_count": 23,
        "url": "/intelligence/signals/42"
      }
    }
  ],
  "edges": [
    { "source": "SIG-000042", "target": "DEC-000123", "edge_type": "influenced" },
    { "source": "DEC-000123", "target": "OUT-000001", "edge_type": "resulted_in" }
  ],
  "generated_at": 1721865600
}
```

## Query Logic

```
1. decision_query.list(None, limit)       → Vec<Decision> (with signal_thread_ids)
2. Extract signal_thread_ids → signal_query.batch_titles(ids) → HashMap<id, SignalBriefInput>
3. Extract decision_ids → outcome_query.batch(ids) → HashMap<decision_id, Vec<OutcomeEvent>>
4. build_graph() → GraphResponse
```

MVP scope: Signal → Decision → Outcome. Reflection and Memory deferred to v2.

Each batch query is 1 D1 call. Total ~4 queries regardless of N.

## GraphProjectionService

**File:** `crates/application/src/graph.rs`

```rust
pub struct GraphProjectionService<S> {
    store: S,
}

impl<S> GraphProjectionService<S>
where
    S: store::DecisionQueryService
        + store::SignalQueryService
        + store::BatchSignalQueryService
        + store::OutcomeQueryService,
{
    pub async fn build_decision_graph(&self, limit: u32) -> Result<GraphResponse, StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;

        // 1. Load recent decisions
        let decisions = self.store.list_decisions(None, limit).await?;
        if decisions.is_empty() {
            return Ok(GraphResponse::empty("decision-graph"));
        }

        let root_id = format!("DEC-{:06}", decisions[0].id);

        // 2. Collect IDs for batch loading
        let decision_ids: Vec<i64> = decisions.iter().map(|d| d.id).collect();
        let signal_ids: Vec<i64> = decisions.iter().filter_map(|d| d.signal_thread_id).collect();

        // 3. Batch load signals (using existing batch_evidence or get_active_signal_threads)
        let mut signal_map: HashMap<i64, (String, f64)> = HashMap::new();
        if !signal_ids.is_empty() {
            let threads = self.store.get_active_signal_threads(signal_ids.len() as u32).await?;
            for t in &threads {
                if signal_ids.contains(&t.thread_id) {
                    signal_map.insert(t.thread_id, (t.title.clone(), t.health_score));
                }
            }
        }

        // 4. Build nodes
        let mut decision_nodes: Vec<GraphNode> = decisions.iter().map(|d| GraphNode {
            id: format!("DEC-{:06}", d.id),
            entity_id: d.id,
            node_type: GraphNodeType::Decision,
            title: d.title.clone(),
            data: GraphNodeData { confidence: Some(d.confidence), status: Some(d.status.clone()), source_count: None, article_count: None, url: Some(format!("/intelligence/decisions/{}", d.id)) },
        }).collect();

        let mut signal_nodes: Vec<GraphNode> = Vec::new();
        let mut edges: Vec<GraphEdge> = Vec::new();

        for d in &decisions {
            if let Some(sid) = d.signal_thread_id {
                if let Some((title, health)) = signal_map.get(&sid) {
                    signal_nodes.push(GraphNode {
                        id: format!("SIG-{:06}", sid),
                        entity_id: sid,
                        node_type: GraphNodeType::Signal,
                        title: title.clone(),
                        data: GraphNodeData { confidence: Some(*health), status: None, source_count: None, article_count: None, url: Some(format!("/intelligence/signals/{}", sid)) },
                    });
                    edges.push(GraphEdge { source: format!("SIG-{:06}", sid), target: format!("DEC-{:06}", d.id), edge_type: GraphEdgeType::Influenced });
                }
            }
        }

        let mut nodes = Vec::new();
        nodes.extend(signal_nodes);
        nodes.extend(decision_nodes);

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

## Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `crates/application/src/graph.rs` | Create | Types + GraphProjectionService |
| `crates/application/src/lib.rs` | Modify | Add `pub mod graph;` |
| `crates/api/src/routes/graph.rs` | Create | HTTP handler for the projection endpoint |
| `crates/api/src/lib.rs` | Modify | Register `/api/projections/decision-graph` route |

## Verification

```bash
cargo check --workspace
cargo test --workspace
```

Then verify via curl (after deploy):
```bash
curl https://sulix-feed-worker.weixc0856.workers.dev/api/projections/decision-graph?limit=5
```

Expected: `{ projection: "decision-graph", root: {...}, nodes: [...], edges: [...], generated_at: ... }`
