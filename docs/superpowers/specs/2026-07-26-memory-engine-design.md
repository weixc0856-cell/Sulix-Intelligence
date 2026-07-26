# Memory Engine — Sprint 5.5 Design Spec

## Context

Sprint 5.4 完成了 Reflection Engine（Decision Learning Loop）。Sprint 5.5 实现 **Memory Consolidation Loop**——将 Reflection 提炼后的经验晋升为 Sulix 的长期认知资产。

### Sulix Cognitive Loop

```
Observe (RSS/Signal)
    ↓
Understand (Signal Engine)
    ↓
Decide (Decision Loop)
    ↓
Act → Outcome
    ↓
Reflect (Reflection Engine)    ← Sprint 5.4
    ↓
Consolidate Memory             ← Sprint 5.5
    ↓
Improve Future Decisions (Agent)
```

### 定位

Memory 不是 Reflection 的 ETL 副本。它是 **Cognitive Knowledge Layer**——可追溯、可演化的持久信念（Belief Object + Evidence Lineage）。

| Layer | 职责 | 产物 |
|-------|------|------|
| Archive | 保存事实 | R2 objects |
| Reflection | 解释经验 | Lessons + Rules |
| **Memory** | **提炼长期认知信念** | **可追溯、可更新、可衰减的 Belief Object** |

### 核心原则

1. **独立 cron worker** — `memory::process_pending`，每日一次
2. **结构化语义记忆** — 不是 vector dump，是有 schema 的 Belief Object
3. **可追溯** — 每条 Memory 有完整 lineage（Reflection[] + Decision[] + Outcome[] + Signal[]）
4. **评分晋升 + 衰减** — 不是所有经验都值得记忆；记忆会随时间衰减
5. **Graveyard 不删除** — 失败的经验也是资产，标记 archived 而非 discard
6. **Outbox 一致性** — 沿用 Event + Outbox 模式，保证 D1/R2 一致
7. **Origin 溯源** — 区分 Explicit（用户明确）/ Derived（AI 推理）/ Observed（模式发现）/ Learned（强化）

---

## Section 1: Architecture

### 模块结构

```
crates/memory-engine/

├── Cargo.toml
├── src/
│   ├── lib.rs              ← pub
│   ├── candidate.rs        ← CandidateExtractor：从 event_archive_index 读取 ReflectionGenerated
│   ├── evaluator.rs        ← MemoryEvaluator：评分 + Promotion Gate + 衰减
│   ├── promotion.rs        ← MemoryPromotion：Outbox → D1 + R2 + EventStore
│   └── worker.rs           ← Cron 入口：process_pending
```

### 数据流

```
ReflectionGenerated Event (in event_archive_index + R2)
    │
    └── Memory Consolidation Cron (Daily, 01:00)
            │
            ├── 1. CandidateExtractor
            │       └── query event_archive_index: aggregate_type='reflection'
            │           AND occurred_at > last_run (KV key: memory:last_run)
            │
            ├── 2. MemoryEvaluator
            │       ├── Promotion Gate (hard fail-fast):
            │       │   ├── quality_score >= 0.7?
            │       │   ├── outcome exists?
            │       │   ├── evidence exists?
            │       │   └── (rules >= 1 OR lessons >= 1)?
            │       │
            │       └── Scoring (pass gate → score):
            │               promotion_score = 0.25*confidence + 0.20*recurrence
            │                                + 0.20*impact + 0.20*evidence + 0.15*stability
            │               ├── >0.75 → promote (status=active)
            │               ├── 0.4-0.75 → pending (human review)
            │               └── <0.4 → archived (graveyard, not deleted)
            │
            └── 3. MemoryPromotion
                    ├── Outbox (event:memory → archive worker)
                    ├── D1: INSERT memory_index (status=active)
                    ├── R2: write memory/insights/MEM-{id}.json
                    └── EventStore: append MemoryPromoted event
```

### 文件结构

```
crates/
  memory-engine/                 ← 新建 crate
    Cargo.toml
    src/
      lib.rs
      candidate.rs               ← CandidateExtractor
      evaluator.rs               ← MemoryEvaluator + PromotionScore
      promotion.rs               ← MemoryPromotion
      worker.rs                  ← process_pending (cron entry)

  store/src/
    models/memory.rs             ← 新建: Memory, NewMemory types
    domain/memory/
      mod.rs                     ← registry
      crud.rs                    ← D1Store memory CRUD
    backend.rs                   ← StoreBackend memory methods
    d1_delegate.rs               ← delegate
    memory/mod.rs + backend.rs    ← MemoryStore impl

  worker-entry/src/
    jobs/memory.rs               ← Cron 调度入口

migrations/
  0025_memory_engine.sql         ← memory_index 表
```

---

## Section 2: Data Model

### MemoryOrigin Enum

```rust
pub enum MemoryOrigin {
    Explicit,     // user explicitly stated
    Derived,      // AI inference from Reflection
    Observed,     // behavioral pattern discovery
    Learned,      // reinforced by multiple feedback cycles
}
```

### MemoryType Enum

```rust
pub enum MemoryType {
    StrategicPattern,
    DomainKnowledge,
    DecisionHeuristic,
    PersonalPreference,
    FailurePattern,
}
```

### Memory Status State Machine

```
candidate → pending (review) → active → deprecated → archived
  ↓            ↓                   ↓
archived    archived            archived (stable)
```

- **Discard 不存在** — 所有经验至少进入 archive（graveyard）
- **Archived** ≠ 删除，保留 lineage 用于分析

### D1 Index: `memory_index`

```sql
CREATE TABLE IF NOT EXISTS memory_index (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    memory_type         TEXT NOT NULL,       -- strategic_pattern, domain_knowledge, ...
    memory_origin       TEXT NOT NULL DEFAULT 'derived',  -- explicit | derived | observed | learned
    statement           TEXT NOT NULL,
    confidence          REAL NOT NULL DEFAULT 0.0,
    stability_score     REAL,                -- 0.0-1.0, for calculating effective_confidence
    confidence_updated_at INTEGER,           -- for time-based decay calculation
    memory_sources      TEXT,                -- JSON array of {type, id}[]
    artifact_key        TEXT,                -- memory/insights/MEM-{id}.json
    status              TEXT NOT NULL DEFAULT 'candidate',  -- candidate | pending | active | deprecated | archived
    usage_count         INTEGER DEFAULT 0,
    validation_count    INTEGER DEFAULT 0,
    promoted_at         INTEGER NOT NULL DEFAULT (unixepoch()),
    deprecated_at       INTEGER,
    last_used_at        INTEGER,
    created_at          INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(artifact_key)
);

CREATE INDEX IF NOT EXISTS idx_memory_type ON memory_index(memory_type);
CREATE INDEX IF NOT EXISTS idx_memory_status ON memory_index(status);
CREATE INDEX IF NOT EXISTS idx_memory_origin ON memory_index(memory_origin);
```

### Rust Types

```rust
pub struct Memory {
    pub id: i64,
    pub memory_type: String,
    pub memory_origin: String,
    pub statement: String,
    pub confidence: f64,
    pub stability_score: Option<f64>,
    pub confidence_updated_at: Option<i64>,
    pub memory_sources: Vec<MemorySourceRef>,
    pub artifact_key: Option<String>,
    pub status: String,
    pub usage_count: i64,
    pub validation_count: i64,
    pub promoted_at: i64,
    pub deprecated_at: Option<i64>,
    pub last_used_at: Option<i64>,
    pub created_at: i64,
}

pub struct MemorySourceRef {
    pub source_type: String,   // reflection | decision | outcome | signal
    pub source_id: String,     // "REF-000001"
}

pub struct PromotionScore {
    pub confidence: f32,
    pub recurrence: f32,
    pub impact: f32,
    pub evidence: f32,
    pub stability: f32,
    pub total: f32,
}
```

### R2 Artifact Schema（Belief Object）

`memory/insights/MEM-000001.json`:

```json
{
  "schema_version": 1,
  "artifact_type": "memory",
  "memory_id": "MEM-000001",
  "memory_type": "strategic_pattern",
  "memory_origin": "derived",

  "claim": {
    "statement": "Technology breakthroughs do not guarantee commercial adoption",
    "type": "heuristic"
  },

  "belief": {
    "confidence": 0.85,
    "stability": 0.7,
    "effective_confidence": 0.82
  },

  "lineage": {
    "decisions": ["DEC-000001"],
    "outcomes": ["OUT-000001"],
    "reflections": ["REF-000001"],
    "signals": ["SIG-001"]
  },

  "promotion": {
    "score": 0.82,
    "promotion_criteria": {
      "quality_score": 0.85,
      "has_outcome": true,
      "has_evidence": true
    }
  },

  "usage": {
    "times_used": 0,
    "last_used": null
  },

  "created_at": 1710000000
}
```

---

## Section 3: Promotion Evaluation

### Scoring Model

```
promotion_score =
  0.25 × confidence         (from Reflection quality_score)
+ 0.20 × recurrence         (how often this pattern appears)
+ 0.20 × impact             (severity of outcome)
+ 0.20 × evidence           (how well-grounded)
+ 0.15 × stability          (temporal stability: days_seen / observation_window)
```

| Score | Status |
|-------|--------|
| >0.75 | active |
| 0.4-0.75 | pending（human review） |
| <0.4 | archived（graveyard） |

### Promotion Gate（Hard fail-fast before scoring）

```
quality_score >= 0.7?        → archived
outcome exists?              → archived
evidence exists?             → archived
(rules >= 1 OR lessons >= 1)? → archived
```

### Confidence Decay

```rust
fn effective_confidence(memory: &Memory, now: i64) -> f64 {
    let days_since = (now - memory.confidence_updated_at.unwrap_or(memory.promoted_at)) / 86400;
    let lambda = match memory.memory_type.as_str() {
        "strategic_pattern" => 0.002,   // ~1 year halflife
        "domain_knowledge" => 0.001,    // ~2 years
        "decision_heuristic" => 0.003,  // ~8 months
        "personal_preference" => 0.0005, // ~4 years
        "failure_pattern" => 0.002,     // ~1 year
        _ => 0.001,
    };
    memory.confidence * (-lambda * days_since as f64).exp()
}
```

---

## Section 4: Cron Implementation

### 调度

```
ingestion → gc → signal → briefing → archive → reflection → memory
```

频率：每日一次（由 `memory:last_run` KV key 控制，类似 `signal_engine:last_run`）

### Batch 策略

```rust
const MEMORY_BATCH_SIZE: u32 = 50;  // configurable via env MEMORY_BATCH_SIZE
```

### 幂等

`UNIQUE(artifact_key)` 防止同一 Reflection 被重复记忆。

### Outbox Consistency

MemoryPromotion 不直接写 D1 + R2。沿用 Event + Outbox 模式：

```
MemoryPromotion
    │
    ├── D1: INSERT memory_index
    ├── Outbox: event:memory (EventEnvelope → MemoryPromoted)
    ├── Outbox: archive:memory (artifact JSON)
    └── Archive worker → R2 + EventStore
```

---

## Section 5: Sprint 边界

### Sprint 5.5（当前）做

- `memory_index` 表（含 lineage, origin, stability, decay 字段）
- `crates/memory-engine` crate（CandidateExtractor + MemoryEvaluator + MemoryPromotion）
- StoreBackend memory CRUD
- Cron worker（每日，独立，batch=50）
- `MemoryPromoted` 事件
- R2 `memory/insights/MEM-{id}.json`（Belief Object 格式）
- Outbox 一致性（event:memory + archive:memory）

### Sprint 5.5 不做

- Vector/Embedding Memory（后续 sprint）
- Memory 查询/检索 API（后续 sprint）
- Event-driven consumer（后续 sprint）
- Agent Memory Retrieval（Sprint 5.6+）
- Confidence decay cron job（实现函数，但不调度）

---

## Section 6: Verification

1. `cargo check --workspace` + `cargo test --workspace`
2. **Candidate extraction test**: event_archive_index → filter aggregate_type='reflection' → candidates
3. **Promotion gate test**: quality < 0.7 → archived; outcome missing → archived
4. **Scoring test**: confidence=0.9, recurrence=0.5, stability=0.8 → total=0.9*0.25+0.5*0.2+0.8*0.15=0.445
5. **Confidence decay test**: days_since=90, lambda=0.002 → factor=exp*(-0.18)=0.835
6. **Durability test**: outbox → archive worker → D1 + R2 consistent
7. **Idempotency test**: same reflection processed twice → one memory entry (UNIQUE)
8. **Status lifecycle test**: candidate → active → deprecated → archived
