# Cognitive Context Engine (Sprint 5.6) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development (recommended) to implement this plan task-by-task.

**Goal:** Build the Cognitive Context Engine — transforms Decision/Reflection/Memory data into structured, ranked, traceable AgentContext for LLM consumption.

**Architecture:** New `crates/context-engine/` crate with IntentParser → RetrievalPlanner → parallel Retrievers → RankingStrategies → ContextAssembler. Internal API only. No embedding, no chat UI.

**Tech Stack:** Rust + Cloudflare Workers + D1 + existing StoreBackend types

**Spec reference:** `docs/superpowers/specs/2026-07-26-context-engine-design.md`

---

## File Structure

### New files:
- `migrations/0026_context_snapshots.sql`
- `crates/store/src/models/context_snapshot.rs` — ContextSnapshot types
- `crates/store/src/domain/context_snapshot/mod.rs`
- `crates/store/src/domain/context_snapshot/crud.rs` — D1Store CRUD
- `crates/context-engine/Cargo.toml`
- `crates/context-engine/src/lib.rs`
- `crates/context-engine/src/types.rs` — AgentContext, Intent, CognitiveStage, etc.
- `crates/context-engine/src/intent.rs` — IntentParser
- `crates/context-engine/src/planner.rs` — RetrievalPlanner
- `crates/context-engine/src/retriever.rs` — Decision/Reflection/Memory retrievers
- `crates/context-engine/src/pattern.rs` — PatternDetector
- `crates/context-engine/src/ranking.rs` — RankingStrategy trait + per-category impls
- `crates/context-engine/src/assembler.rs` — ContextAssembler
- `crates/context-engine/src/builder.rs` — ContextBuilder facade
- `crates/api/src/routes/context.rs` — POST /api/internal/context

### Existing files to modify:
- `Cargo.toml` (workspace) — add context-engine member + dep
- `crates/store/src/models/mod.rs` — add context_snapshot
- `crates/store/src/domain/mod.rs` — add context_snapshot module
- `crates/store/src/backend.rs` — StoreBackend snapshot methods
- `crates/store/src/d1_delegate.rs` — delegation
- `crates/store/src/memory/mod.rs` — MemoryStore fields
- `crates/store/src/memory/backend.rs` — MemoryStore impl
- `crates/api/Cargo.toml` — add context-engine dep
- `crates/api/src/lib.rs` — register context route

---

## Task Plan

### Task 1: Migration — context_snapshots table

**Files:**
- Create: `migrations/0026_context_snapshots.sql`

```sql
CREATE TABLE IF NOT EXISTS context_snapshots (
    id              TEXT PRIMARY KEY,
    query           TEXT NOT NULL,
    intent          TEXT NOT NULL,
    engine_version  TEXT NOT NULL DEFAULT 'context-engine-v1',
    context_json    TEXT NOT NULL,
    confidence      REAL NOT NULL DEFAULT 0.0,
    user_scope      TEXT,
    created_at      INTEGER NOT NULL DEFAULT (unixepoch())
);
```

Commit.

### Task 2: ContextSnapshot model + StoreBackend

**Files:**
- Create: `crates/store/src/models/context_snapshot.rs`
- Modify: `crates/store/src/models/mod.rs`
- Modify: `crates/store/src/backend.rs`
- Modify: `crates/store/src/d1_delegate.rs`
- Create: `crates/store/src/domain/context_snapshot/mod.rs`
- Create: `crates/store/src/domain/context_snapshot/crud.rs`
- Modify: `crates/store/src/domain/mod.rs`
- Modify: `crates/store/src/memory/mod.rs`
- Modify: `crates/store/src/memory/backend.rs`

Model:
```rust
pub struct ContextSnapshot {
    pub id: String,
    pub query: String,
    pub intent: String,
    pub engine_version: String,
    pub context_json: String,
    pub confidence: f64,
    pub user_scope: Option<String>,
    pub created_at: i64,
}

pub struct NewContextSnapshot {
    pub id: String,
    pub query: String,
    pub intent: String,
    pub context_json: String,
    pub confidence: f64,
    pub user_scope: Option<String>,
}
```

StoreBackend methods:
- `save_context_snapshot(&self, snap: &NewContextSnapshot) -> Result<(), StoreError>`
- `get_context_snapshot(&self, id: &str) -> Result<Option<ContextSnapshot>, StoreError>`

D1Store, MemoryStore, delegate: same pattern as previous sprints.

Commit.

### Task 3: context-engine crate skeleton

**Files:**
- Create: `crates/context-engine/Cargo.toml`
- Create: `crates/context-engine/src/lib.rs`
- Modify: `Cargo.toml` (workspace)

Cargo.toml — depends on `worker`, `store`, `serde`, `serde_json`, `async-trait`.

`lib.rs`:
```rust
pub mod types;
pub mod intent;
pub mod planner;
pub mod retriever;
pub mod pattern;
pub mod ranking;
pub mod assembler;
pub mod builder;
```

Register in workspace Cargo.toml.

Commit.

### Task 4: Types module

**Files:**
- Create: `crates/context-engine/src/types.rs`

All core types:
- `AgentContext` — query, intent, evidence (lineage), decisions, reflections, memories, patterns, confidence, engine_version, generated_at
- `Intent` — intent_type, stage (CognitiveStage), desired_outcome (DesiredOutcome), domain, action, entity
- `CognitiveStage` enum: Explore, Decide, Review, Learn
- `DesiredOutcome` enum: Recommendation, Explanation, Comparison, Prediction
- `ContextEvidence` — source_type, source_id, selection_reason, relevance_score
- `ScoredDecision`, `ScoredReflection`, `ScoredMemory`
- `RankComponents` — query_alignment, confidence, recency, usage_frequency, user_specificity
- `ContextConfidence` — overall, coverage, data_quality, recency, consistency
- `PatternContext` — pattern_type, description, frequency, evidence_refs

Include tests for serialization roundtrip.

Commit.

### Task 5: IntentParser

**Files:**
- Create: `crates/context-engine/src/intent.rs`

```rust
pub fn parse(query: &str) -> Intent;
```

Rule-based keyword matching:
- "should I invest|enter|buy|start" → Explore, Recommendation, domain=investment
- "why did|does|is" → Review, Explanation
- "what did I learn" → Review, Explanation
- "tell me about|what is" → Learn, Explanation

Fallback: pattern_analysis, Learn, Explanation.

Include tests for all 4 intent patterns plus fallback.

Commit.

### Task 6: RetrievalPlanner

**Files:**
- Create: `crates/context-engine/src/planner.rs`

```rust
pub struct RetrievalPlan {
    pub decision_filters: DecisionFilter,
    pub reflection_filters: ReflectionFilter,
    pub memory_filters: MemoryFilter,
    pub detect_patterns: bool,
    pub max_results: u32,
}

pub struct DecisionFilter {
    pub decision_type: Option<String>,
    pub status: Option<String>,
    pub limit: u32,
}

pub struct ReflectionFilter {
    pub status: Option<String>,
    pub min_quality: Option<f64>,
    pub limit: u32,
}

pub struct MemoryFilter {
    pub memory_types: Vec<String>,
    pub status: Option<String>,
    pub min_confidence: Option<f64>,
    pub limit: u32,
}

pub fn plan(intent: &Intent) -> RetrievalPlan;
```

Map Intent fields to filters:
- `intent.domain` → decision_filters.decision_type, memory_filters.memory_types
- `intent.stage == Review` → reflection_filters.status = "generated"

Include test: investment intent → DecisionFilter with decision_type=investment.

Commit.

### Task 7: Retrievers

**Files:**
- Create: `crates/context-engine/src/retriever.rs`

```rust
pub async fn retrieve_decisions<S: StoreBackend>(
    store: &S, plan: &DecisionFilter,
) -> Result<Vec<store::Decision>, String>;

pub async fn retrieve_reflections<S: StoreBackend>(
    store: &S, plan: &ReflectionFilter,
) -> Result<Vec<store::Reflection>, String>;

pub async fn retrieve_memories<S: StoreBackend>(
    store: &S, plan: &MemoryFilter,
) -> Result<Vec<store::Memory>, String>;
```

Each queries the store using existing StoreBackend methods + client-side filtering (since D1 doesn't support dynamic complex WHERE easily).

Include test: retrieve_decisions with store that has matching data → non-empty.

Commit.

### Task 8: PatternDetector

**Files:**
- Create: `crates/context-engine/src/pattern.rs`

```rust
pub fn detect_patterns(decisions: &[store::Decision], reflections: &[store::Reflection]) -> Vec<PatternContext>;
```

Scan decisions for repeated `result` values (via reflections) and decision_type clusters. MVP: group by decision_type, count, flag if >1 failed outcome in same type.

Include test: 2 investment failures → pattern detected.

Commit.

### Task 9: Ranking

**Files:**
- Create: `crates/context-engine/src/ranking.rs`

```rust
pub trait RankingStrategy {
    fn score(&self, components: &RankComponents) -> f64;
}

pub struct DecisionRanking;
pub struct MemoryRanking;
pub struct ReflectionRanking;

pub fn rank_decisions(items: Vec<ScoredDecision>) -> Vec<ScoredDecision>;
pub fn rank_reflections(items: Vec<ScoredReflection>) -> Vec<ScoredReflection>;
pub fn rank_memories(items: Vec<ScoredMemory>) -> Vec<ScoredMemory>;
```

Default formula: 0.30*query_alignment + 0.25*confidence + 0.20*recency + 0.15*usage + 0.10*specificity

Include tests: higher confidence → higher rank; empty → empty.

Commit.

### Task 10: ContextAssembler + ContextBuilder

**Files:**
- Create: `crates/context-engine/src/assembler.rs`
- Create: `crates/context-engine/src/builder.rs`

`assembler.rs`:
```rust
pub fn assemble(
    query: &str, intent: &Intent,
    decisions: Vec<ScoredDecision>,
    reflections: Vec<ScoredReflection>,
    memories: Vec<ScoredMemory>,
    patterns: Vec<PatternContext>,
) -> AgentContext;
```

Compute ContextConfidence: overall from coverage + quality + recency + consistency. Build evidence lineage for each item.

`builder.rs` — ContextBuilder facade:
```rust
pub struct ContextBuilder<S: StoreBackend> {
    store: S,
}

impl<S: StoreBackend> ContextBuilder<S> {
    pub fn new(store: S) -> Self;
    pub async fn build(&self, query: &str, user_scope: Option<String>) -> Result<AgentContext, String>;
}
```

The `build` method: parse intent → plan retrieval → retrieve → detect patterns → rank → assemble → save snapshot → return.

Include assembly test: AgentContext has all 5 sections populated.

Commit.

### Task 11: Internal API route

**Files:**
- Create: `crates/api/src/routes/context.rs`
- Modify: `crates/api/Cargo.toml`
- Modify: `crates/api/src/lib.rs`

```rust
pub async fn internal_context(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    // POST /api/internal/context
    // Body: { query, user_scope?, options? }
    // Returns: AgentContext JSON
}
```

Route: `.post_async("/api/internal/context", routes::context::internal_context)`

Commit.

### Task 12: Full compilation + test

`cargo check --workspace` + `cargo test --workspace`. Fix issues. Commit final.
