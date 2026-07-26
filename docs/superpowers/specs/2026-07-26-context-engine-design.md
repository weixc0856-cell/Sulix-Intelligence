# Cognitive Context Engine — Sprint 5.6 Design Spec

## Context

Sprint 5.0-5.5 完成了完整的 Sulix Cognitive Loop（Signal → Decision → Outcome → Reflection → Memory）。Sprint 5.6 构建 **Cognitive Context Engine**——将用户的决策历史、反思经验和长期记忆转化为结构化上下文，供 Agent 推理使用。

### 定位

不是 RAG/搜索服务。而是 Sulix 认知数据的"上下文层"：

```
User Question
    ↓
Cognitive Context Engine (Sprint 5.6)
    ↓
    ├── Relevant Decisions
    ├── Related Reflections
    ├── Activated Memories
    └── Identified Patterns
    ↓
Agent / Chat / Advisor (Sprint 5.7)
```

### 核心原则

1. **Retrieval ≠ Context** — 不只是搜索，而是理解用户意图后激活最相关的认知资产
2. **结构化优先** — 先做 structured retrieval（type/domain/status 过滤），不做 embedding
3. **Ranking 模型** — 综合 semantic_similarity + confidence + recency + usage + user_specificity
4. **Context Snapshot** — 每次上下文生成可追溯、可复现
5. **内部 API** — 只暴露给 Sulix 内部组件，不直接对外
6. **不做 Chat UI、不做 Agent API、不做 Embedding**

---

## Section 1: Architecture

### 模块结构

```
crates/context-engine/

├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── types.rs                ← AgentContext, Intent, ContextConfidence, etc.
│   ├── intent.rs               ← IntentClassifier (rule-based for MVP)
│   ├── retriever.rs            ← MemoryRetriever + DecisionRetriever + ReflectionRetriever
│   ├── ranking.rs              ← ContextRanking model
│   ├── assembler.rs            ← ContextAssembler → AgentContext
│   └── builder.rs              ← ContextBuilder (facade)
```

### 数据流

```
User Query ("Should I invest in AI startups?")
    ↓
1. IntentClassifier
    ├── intent: decision_support
    ├── domain: investment
    ├── action: evaluate
    └── entity: AI startup
    ↓
2. Retrieval (parallel)
    ├── DecisionRetriever
    │   └── query decisions WHERE decision_type='investment' AND status!='draft'
    │       ORDER BY similarity DESC LIMIT 10
    ├── ReflectionRetriever
    │   └── query reflections WHERE domain='investment' AND status='generated'
    │       ORDER BY quality_score DESC LIMIT 10
    ├── MemoryRetriever
    │   └── query memory_index WHERE (memory_type='decision_heuristic' OR 'strategic_pattern')
    │       AND status='active' ORDER BY confidence DESC LIMIT 10
    └── PatternDetector
        └── aggregate: repeated failure patterns in matching decisions
    ↓
3. Ranking
    ├── score = 0.30*semantic + 0.25*confidence + 0.20*recency + 0.15*usage + 0.10*specificity
    └── top-k per category
    ↓
4. ContextAssembler
    ├── build AgentContext
    ├── compute ContextConfidence
    └── persist ContextSnapshot
    ↓
AgentContext (structured, ready for LLM prompt)
```

### 文件结构

```
crates/
  context-engine/               ← 新建 crate

  api/src/
    routes/context.rs           ← POST /api/internal/context

  store/src/
    models/context_snapshot.rs  ← ContextSnapshot
    domain/context_snapshot/
      crud.rs                   ← D1Store CRUD
    backend.rs                  ← StoreBackend methods
    d1_delegate.rs, memory/     ← stubs

migrations/
  0026_context_snapshots.sql   ← context_snapshots 表
```

---

## Section 2: Data Model

### ContextSnapshot 表

```sql
CREATE TABLE IF NOT EXISTS context_snapshots (
    id              TEXT PRIMARY KEY,           -- "CTX-{timestamp}-{hash}"
    query           TEXT NOT NULL,
    intent          TEXT NOT NULL,
    domain          TEXT,
    context_json    TEXT NOT NULL,              -- full AgentContext JSON
    confidence      REAL NOT NULL DEFAULT 0.0,
    created_at      INTEGER NOT NULL DEFAULT (unixepoch())
);
```

### Rust Types

```rust
pub struct AgentContext {
    pub query: String,
    pub intent: Intent,
    pub decisions: Vec<ScoredDecision>,
    pub reflections: Vec<ScoredReflection>,
    pub memories: Vec<ScoredMemory>,
    pub patterns: Vec<PatternContext>,
    pub confidence: ContextConfidence,
    pub generated_at: i64,
}

pub struct Intent {
    pub intent_type: String,     // decision_support | reflection | pattern_analysis
    pub domain: Option<String>,  // investment | career | product | ...
    pub action: Option<String>,  // evaluate | understand | compare
    pub entity: Option<String>,
}

pub struct ScoredMemory {
    pub memory: Memory,
    pub relevance_score: f64,
    pub rank_components: RankComponents,
}

pub struct RankComponents {
    pub semantic_similarity: f64,
    pub confidence: f64,
    pub recency: f64,
    pub usage_frequency: f64,
    pub user_specificity: f64,
}

pub struct ContextConfidence {
    pub overall: f64,
    pub data_quality: f64,       // how much relevant data exists
    pub recency: f64,            // how recent the data is
    pub consistency: f64,        // do the sources agree?
}

pub struct PatternContext {
    pub pattern_type: String,     // failure_pattern | success_pattern | recurring_theme
    pub description: String,
    pub frequency: u32,
    pub evidence_refs: Vec<String>,
}
```

---

## Section 3: Intent Classification

### MVP: Rule-based + Keyword

```
"Should I invest in X" → intent=decision_support, domain=investment, action=evaluate
"Why did X fail"       → intent=reflection, action=understand
"What did I learn from X" → intent=reflection, action=analyze
"Tell me about X"      → intent=pattern_analysis, action=explore
```

Future: LLM-based classifier. For Sprint 5.6, keyword + domain dictionary is sufficient.

```rust
pub fn classify(query: &str) -> Intent {
    // Keyword matching → (intent_type, domain, action, entity)
    // Returns Intent with best-effort parsing
}
```

---

## Section 4: Ranking Model

```
relevance_score =
  0.30 * semantic_similarity
+ 0.25 * confidence
+ 0.20 * recency              (normalized: days_since / max_days)
+ 0.15 * usage_frequency      (normalized: usage_count / max_count)
+ 0.10 * user_specificity     (1.0 if user's own data, 0.5 if derived)
```

Semantic similarity for MVP: token overlap + domain matching (no embedding).

---

## Section 5: Sprint 边界

### 做

- `crates/context-engine/` — ContextBuilder, IntentClassifier, Retrieval, Ranking, Assembler
- `ContextSnapshot` 表 + CRUD
- `POST /api/internal/context` （内部，不对外暴露）
- Structured retrieval from decisions, reflections, memory_index
- Ranking model
- Pattern detection（重复出现的 failure/success themes）
- Configurable top-k per category

### 不做

- Chat UI
- LLM Conversation / Agent Response
- External Agent API
- Embedding / Vector Search
- LLM Intent Classification（rule-based MVP）

---

## Section 6: Verification

1. `cargo check --workspace` + `cargo test --workspace`
2. **Intent test**: "Should I invest in AI?" → intent=decision_support, domain=investment
3. **Retrieval test**: matching decisions returned for domain=investment
4. **Ranking test**: higher confidence → higher rank
5. **Context assembly test**: AgentContext has all 5 sections populated
6. **Snapshot persistence test**: context written + retrievable by id
