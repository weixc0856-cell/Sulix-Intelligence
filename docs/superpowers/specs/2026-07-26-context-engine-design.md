# Cognitive Context Engine — Sprint 5.6 Design Spec

## Context

Sprint 5.0-5.5 完成了完整的 Sulix Cognitive Loop。Sprint 5.6 构建 **Cognitive Context Engine**——将用户的决策历史、反思经验和长期记忆转化为 Agent 可消费的认知上下文快照。

### 定位

Context Engine 不是"高级搜索"。它是 **Agent 看到的用户认知世界**——一次推理所需的完整认知状态。

```
User Question
    ↓
Query Understanding (intent + stage + desired outcome)
    ↓
Retrieval Planner
    ↓
    ├── Decision History
    ├── Reflection Knowledge
    ├── Memory Beliefs
    └── Pattern Detection
    ↓
Ranking Engine (per-category strategy)
    ↓
Context Assembler
    ↓
Cognitive Context Snapshot (immutable, versioned, traceable)
    ↓
Sprint 5.7 Agent
```

### 核心原则

1. **Retrieval ≠ Context** — 不只是搜索，而是理解意图后激活最相关的认知资产
2. **结构化优先** — 不做 embedding，先做 structured retrieval + ranking
3. **每步可追溯** — ContextSnapshot 包含 lineage、版本、engine_version
4. **内部 API** — 只暴露给 Sulix 内部组件
5. **不做 Chat UI、不做 Agent Action、不做 Embedding**

---

## Section 1: Architecture

### 模块结构

```
crates/context-engine/

├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── types.rs                ← AgentContext, Intent, CognitiveStage, DesiredOutcome, etc.
│   ├── intent.rs               ← IntentParser (rule-based MVP)
│   ├── planner.rs              ← RetrievalPlanner (intent → retrieval plan)
│   ├── retriever.rs            ← DecisionRetriever + ReflectionRetriever + MemoryRetriever
│   ├── pattern.rs              ← PatternDetector (repeated themes in results)
│   ├── ranking.rs              ← RankingStrategy trait + per-category strategies
│   ├── assembler.rs            ← ContextAssembler → AgentContext
│   └── builder.rs              ← ContextBuilder (facade)
```

### 数据流

```
User Query
    ↓
1. IntentParser
    ├── intent_type (decision_support|reflection|pattern_analysis)
    ├── stage (explore|decide|review|learn)
    ├── desired_outcome (recommendation|explanation|comparison)
    ├── domain, action, entity
    ↓
2. RetrievalPlanner
    └── intent → structured retrieval plan:
        ├── decision: { domain, limit, order_by }
        ├── reflection: { domain, min_quality }
        └── memory: { types, status }
    ↓
3. Retrieval (parallel)
    ├── DecisionRetriever → Vec<Decision>
    ├── ReflectionRetriever → Vec<Reflection>
    ├── MemoryRetriever → Vec<Memory>
    └── PatternDetector → Vec<PatternContext>
    ↓
4. Ranking
    ├── per-category RankingStrategy
    ├── combined relevance_score per item
    └── top-k per category
    ↓
5. ContextAssembler
    ├── build AgentContext (with evidence lineage)
    ├── compute ContextConfidence (overall + coverage + quality + recency + consistency)
    └── persist ContextSnapshot (versioned + engine_version)
```

---

## Section 2: Data Model

### ContextSnapshot 表

```sql
CREATE TABLE IF NOT EXISTS context_snapshots (
    id              TEXT PRIMARY KEY,           -- "CTX-{timestamp}-{hash}"
    query           TEXT NOT NULL,
    intent          TEXT NOT NULL,              -- JSON: Intent
    engine_version  TEXT NOT NULL DEFAULT 'context-engine-v1',
    context_json    TEXT NOT NULL,              -- full AgentContext JSON
    confidence      REAL NOT NULL DEFAULT 0.0,
    user_scope      TEXT,                       -- JSON: { user_id, profile_id }
    created_at      INTEGER NOT NULL DEFAULT (unixepoch())
);
```

### Rust Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContext {
    pub query: String,
    pub intent: Intent,
    pub evidence: Vec<ContextEvidence>,  // lineage — why each item was selected
    pub decisions: Vec<ScoredDecision>,
    pub reflections: Vec<ScoredReflection>,
    pub memories: Vec<ScoredMemory>,
    pub patterns: Vec<PatternContext>,
    pub confidence: ContextConfidence,
    pub engine_version: String,
    pub generated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    pub intent_type: String,        // decision_support | reflection | pattern_analysis
    pub stage: CognitiveStage,
    pub desired_outcome: DesiredOutcome,
    pub domain: Option<String>,
    pub action: Option<String>,     // evaluate | understand | compare
    pub entity: Option<String>,
}

pub enum CognitiveStage { Explore, Decide, Review, Learn }
pub enum DesiredOutcome { Recommendation, Explanation, Comparison, Prediction }

/// Lineage — traces why a piece of data was included in context.
pub struct ContextEvidence {
    pub source_type: String,        // decision | reflection | memory
    pub source_id: String,
    pub selection_reason: String,
    pub relevance_score: f64,
}

pub struct ScoredDecision {
    pub decision_id: String,
    pub relevance_score: f64,
    pub rank_components: RankComponents,
    pub evidence: ContextEvidence,
}

pub struct RankComponents {
    pub query_alignment: f64,       // token overlap + domain match (MVP); embedding (future)
    pub confidence: f64,
    pub recency: f64,
    pub usage_frequency: f64,
    pub user_specificity: f64,
}

pub struct ContextConfidence {
    pub overall: f64,
    pub coverage: f64,              // how much personal data exists for this query
    pub data_quality: f64,
    pub recency: f64,
    pub consistency: f64,           // do the sources agree?
}

pub struct PatternContext {
    pub pattern_type: String,       // failure_pattern | success_pattern | recurring_theme
    pub description: String,
    pub frequency: u32,
    pub evidence_refs: Vec<String>,
}
```

---

## Section 3: Intent + Retrieval Planner

### IntentParser (rule-based MVP)

```
"Should I invest in X" → decision_support, Evaluate, Explore, Recommendation
"Why did X fail"       → reflection, Understand, Review, Explanation
"What did I learn"     → reflection, Understand, Review, Explanation
"Tell me about X"      → pattern_analysis, Explore, Learn, Explanation
```

Future: LLM-based extraction. Interface stays same.

### RetrievalPlanner

```rust
pub struct RetrievalPlan {
    pub decision_filter: DecisionFilter,
    pub reflection_filter: ReflectionFilter,
    pub memory_filter: MemoryFilter,
    pub detect_patterns: bool,
    pub max_results: u32,
}

impl RetrievalPlanner {
    pub fn plan(intent: &Intent) -> RetrievalPlan {
        // Map intent fields to structured queries:
        // domain → decision_type, memory_type filter
        // action → ranking weight priorities
        // stage → recency weights
    }
}
```

---

## Section 4: Ranking

### RankingStrategy trait

```rust
pub trait RankingStrategy {
    fn score(&self, item: &RankComponents) -> f64;
}

// Decision: specificity ↑, recency ↑
pub struct DecisionRanking;

// Memory: confidence ↑, stability ↑
pub struct MemoryRanking;

// Reflection: quality_score ↑, recency ↑
pub struct ReflectionRanking;
```

### Default formula

```
relevance_score =
  0.30 * query_alignment
+ 0.25 * confidence
+ 0.20 * recency
+ 0.15 * usage_frequency
+ 0.10 * user_specificity
```

Each category can override weights. `query_alignment` = token overlap + domain match for MVP.

---

## Section 5: Sprint 边界

### 做

- `crates/context-engine/` — ContextBuilder, IntentParser, RetrievalPlanner, Retrievers, Ranking, PatternDetector, Assembler
- `ContextSnapshot` 表 + CRUD + version + engine_version
- `POST /api/internal/context` (internal, with user_scope)
- Structured retrieval from decisions, reflections, memory_index
- Per-category RankingStrategy trait + default impls
- Pattern detection (repeated failure/success themes)
- ContextEvidence lineage (traceable selection)

### 不做

- Chat UI / Agent Response / Agent Actions
- External Agent API
- Embedding / Vector Search
- LLM Intent Classification (rule-based MVP)

---

## Section 6: Verification

1. `cargo check --workspace` + `cargo test --workspace`
2. **Intent test**: "Should I invest in AI?" → intent=decision_support, stage=Explore
3. **Planner test**: intent=investment → plan filters by domain=investment
4. **Retrieval test**: matching decisions returned for domain filter
5. **Ranking test**: higher confidence → higher rank per category
6. **Pattern test**: repeated failure types detected across decisions
7. **Context assembly test**: AgentContext has evidence lineage populated
8. **Snapshot persistence test**: context written + retrievable by id with version
