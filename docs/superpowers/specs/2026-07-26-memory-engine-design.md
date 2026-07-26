# Memory Engine — Sprint 5.5 Design Spec

## Context

Sprint 5.4 完成了 Reflection Engine（Decision Learning Loop）。Sprint 5.5 实现 **Memory Consolidation Loop**——将 Reflection 提炼后的经验晋升为 Sulix 的长期记忆。

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
Consolidate Memory (Memory Engine)  ← Sprint 5.5
    ↓
Improve Future Decisions (Agent)
```

### 定位

Memory Engine 不是简单搬运 Reflection。它是认知循环中的"巩固阶段"：

| Layer | 职责 | 产物 |
|-------|------|------|
| Archive | 保存事实 | R2 objects |
| Reflection | 解释经验 | Lessons + Rules |
| **Memory** | **提炼长期知识** | **可检索、可追溯的经验** |

### 核心原则

1. **独立 cron worker** — `memory::process_pending` 作为 cron 链的最后一步
2. **结构化语义记忆** — 不是 vector dump，是有 schema 的知识记录
3. **可追溯** — 每条 Memory 必须有 `evidence_refs` 链回原始 Reflection/Decision/Outcome
4. **评分晋升** — 不是所有 Reflection 都值得记忆，Promotion Gate + Scoring
5. **Daily 频率** — Memory Consolidation 类似人类睡眠中的记忆巩固，每日一次而非每小时

---

## Section 1: Architecture

### 模块结构

```
crates/memory-engine/

├── Cargo.toml
├── src/

│   ├── lib.rs              ← pub
│   ├── candidate.rs        ← CandidateExtractor：从 EventStore 读取 ReflectionGenerated
│   ├── evaluator.rs        ← MemoryEvaluator：评分 + Promotion Gate
│   ├── promotion.rs        ← MemoryPromotion：写入 R2 + D1 index
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
            │       ├── quality_score >= 0.7 (硬门槛)
            │       ├── outcome exists
            │       ├── evidence exists
            │       └── promotion_score = 0.3*confidence + 0.3*recurrence + 0.2*impact + 0.2*evidence
            │           ├── >0.75 → promote
            │           ├── 0.4-0.75 → review (pending)
            │           └── <0.4 → discard
            │
            └── 3. MemoryPromotion
                    ├── D1: INSERT memory index
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

### D1 Index: `memory_index`

```sql
CREATE TABLE IF NOT EXISTS memory_index (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    memory_type     TEXT NOT NULL,          -- strategic_pattern, domain_knowledge, decision_heuristic, personal_preference, failure_pattern
    statement       TEXT NOT NULL,          -- "EV startups underestimate charging infrastructure"
    confidence      REAL NOT NULL DEFAULT 0.0,
    source_reflection_id TEXT,              -- "REF-000001"
    source_decision_id   TEXT,              -- "DEC-000001"
    evidence_refs   TEXT,                   -- JSON array: ["DEC-00123", "OUT-00456", "REF-00890"]
    artifact_key    TEXT,                   -- memory/insights/MEM-{id}.json or memory/rules/MEM-{id}.json
    status          TEXT NOT NULL DEFAULT 'active',  -- active | archived
    promoted_at     INTEGER NOT NULL DEFAULT (unixepoch()),
    last_used_at    INTEGER,
    created_at      INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_memory_type ON memory_index(memory_type);
CREATE INDEX IF NOT EXISTS idx_memory_status ON memory_index(status);
```

### R2 Artifact Schema

`memory/insights/MEM-000001.json`:

```json
{
  "schema_version": 1,
  "artifact_type": "memory",
  "memory_id": "MEM-000001",
  "memory_type": "strategic_pattern",
  "statement": "Technology breakthroughs do not guarantee commercial adoption",
  "confidence": 0.85,
  "source_chain": {
    "reflection_id": "REF-000001",
    "decision_id": "DEC-000001",
    "outcome_id": "OUT-000001",
    "signals": ["SIG-001"]
  },
  "evidence_refs": ["DEC-000001", "OUT-000001", "REF-000001"],
  "promotion_score": 0.82,
  "promotion_criteria": {
    "quality_score": 0.85,
    "has_outcome": true,
    "has_evidence": true
  },
  "created_at": 1710000000
}
```

### Rust Types

```rust
pub struct Memory {
    pub id: i64,
    pub memory_type: String,
    pub statement: String,
    pub confidence: f64,
    pub source_reflection_id: Option<String>,
    pub source_decision_id: Option<String>,
    pub evidence_refs: Vec<String>,
    pub artifact_key: Option<String>,
    pub status: String,
    pub promoted_at: i64,
    pub last_used_at: Option<i64>,
    pub created_at: i64,
}

pub struct PromotionScore {
    pub confidence: f32,
    pub recurrence: f32,
    pub impact: f32,
    pub evidence: f32,
    pub total: f32,
}
```

### MemoryType Enum

```rust
pub enum MemoryType {
    StrategicPattern,      // recurring strategic signals
    DomainKnowledge,       // domain-specific insights
    DecisionHeuristic,     // rules of thumb for decisions
    PersonalPreference,    // user's personal tendencies
    FailurePattern,        // recurring failure modes
}
```

---

## Section 3: Promotion Evaluation

### Scoring Model

```
promotion_score =
  0.3 × confidence         (from Reflection quality_score)
+ 0.3 × recurrence         (how often this pattern appears)
+ 0.2 × impact             (severity of outcome)
+ 0.2 × evidence           (how well-grounded)
```

| Score | Action |
|-------|--------|
| >0.75 | Promote → R2 + D1 index |
| 0.4-0.75 | Review (status=pending, human review) |
| <0.4 | Discard (not worth remembering) |

### Promotion Gate（硬门槛）

Fail-fast before scoring:

```
quality_score >= 0.7?        → fail → discard
outcome exists?              → fail → discard
evidence exists?             → fail → discard
(rules >= 1 OR lessons >= 1)? → fail → discard
```

---

## Section 4: Cron Implementation

### 调度

在 cron 链中加 `memory::process_pending`，运行于 `reflection` 之后：

```
ingestion → gc → signal → briefing → archive → reflection → memory
```

频率：每日一次（由 `memory:last_run` KV key 控制，类似 `signal_engine:last_run`）

### Batch 策略

一次最多处理 20 个 ReflectionGenerated 事件（EventStore 查询 LIMIT 20）

### 幂等

`UNIQUE(source_reflection_id)` 防止同一 Reflection 被重复记忆。

---

## Section 5: Sprint 边界

### Sprint 5.5（当前）做

- `memory_index` 表
- `crates/memory-engine` crate（CandidateExtractor + MemoryEvaluator + MemoryPromotion）
- StoreBackend memory CRUD
- Cron worker（每日，独立）
- `MemoryPromoted` 事件
- R2 `memory/insights/MEM-{id}.json`

### Sprint 5.5 不做

- Vector/Embedding Memory（后续 sprint）
- Memory 查询 API（后续 sprint）
- Event-driven consumer（后续 sprint）
- Agent Memory Retrieval（Sprint 5.6+）

---

## Section 6: Verification

1. `cargo check --workspace` + `cargo test --workspace`
2. **Candidate extraction test**: EventStore → filter ReflectionGenerated → extract candidates
3. **Promotion gate test**: quality < 0.7 → discard; outcome missing → discard
4. **Scoring test**: confidence=0.9, recurrence=0.5 → correct total
5. **Durability test**: R2 write + D1 index consistent
6. **Idempotency test**: same reflection processed twice → one memory entry
