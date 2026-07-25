# Reflection Engine — Sprint 5.4 Design Spec

## Context

Sprint 5.0-5.3 建立了完整的 Memory Layer 基础设施（ObjectStore, Artifact Archive, EventStream, Decision Event Sourcing）。Sprint 5.4 补全 Decision Learning Loop 的最后一环：**Reflection**。

### Sulix Learning Loop

```
Signal → Decision → Outcome → Reflection → Memory → Future Decision
                                ↑
                          Sprint 5.4
```

### 核心公式

```
Reflection = Decision × Thesis × Evidence × Outcome → Lessons + Decision Rules
```

### 定位

Reflection 是 Decision Learning Loop 的反馈节点。它将"做出的判断 + 当时依据 + 最终结果"转化为可复用的经验和未来决策规则。不是 AI 通用总结，只做 Decision Postmortem。

---

## Section 1: Core Concepts

### 核心原则

1. **Reflection 不是通用 AI 总结**，只做 Decision Postmortem
2. **一个 ReflectionEngine，两个触发入口**：API Trigger + Cron Trigger
3. **统一 Job 模型**，不分裂执行逻辑
4. **Reflection 不直接等于 Memory**：Reflection → Memory Candidate → Memory Promotion
5. **所有经验可追溯**：Memory → Reflection → Outcome → Decision → Signal

### Aggregate 设计

| Aggregate | Events |
|-----------|--------|
| reflection | ReflectionRequested → ReflectionStarted → ReflectionGenerated / ReflectionFailed |
| memory (Sprint 5.5) | MemoryCandidateCreated → MemoryPromoted / MemoryRejected |

### 存储分层

| Layer | Purpose |
|-------|---------|
| D1 | Reflection state + index（reflections 表） |
| R2 | 完整 Reflection Artifact（memory/reflections/REF-{id}.json） |
| EventStore | Reflection lifecycle events |

---

## Section 2: Implementation Architecture

### 文件结构

```
crates/
  intelligence/reflection-engine/    ← 新建 crate
    Cargo.toml
    src/
      lib.rs                         ← pub
      context.rs                     ← ReflectionContextBuilder
      generator.rs                   ← LLM prompt + parse
      validation.rs                  ← Schema + grounding validator
      service.rs                     ← ReflectionEngine (domain service)

  store/src/
    models/reflection.rs             ← 新建: Reflection, NewReflection
    domain/reflection/
      mod.rs                         ← registry
      crud.rs                        ← D1Store reflection CRUD

  api/src/
    routes/reflection.rs              ← POST /decisions/:id/reflect
    services/mod.rs                   ← 已有
    lib.rs                           ← 注册路由 + services

  worker-entry/src/
    jobs/reflection.rs               ← Cron 扫描 + 触发
    jobs/mod.rs                      ← 注册
    runtime/cron.rs                  ← 加 process_pending

migrations/
  0024_reflection_engine.sql         ← reflections 表
```

### ReflectionEngine（领域服务）

```rust
pub struct ReflectionEngine {
    repository: Box<dyn ReflectionRepository>,
    event_store: Box<dyn EventStore>,
    llm: Box<dyn ReflectionGenerator>,
}
```

各组件职责：
- **ReflectionContextBuilder**: 加载 Decision + Thesis + Evidence + Outcome → `ReflectionContext`
- **ReflectionGenerator**: 调用 LLM → 结构化 Reflection JSON
- **Validation Layer**: Schema + grounding + quality 验证
- **ReflectionPersister**: D1 + Outbox + EventStore（不直接写 R2）

### 不分叉原则

Domain service 永远不直接写 artifact storage。所有 durable projection 通过 state + event + outbox 驱动：

```
D1 INSERT reflection
D1 INSERT outbox (event=ReflectionGenerated)
COMMIT
  ↓
archive worker → EventStore append → R2 artifact
```

---

## Section 3: Data Model & Event Schema

### EventEnvelope

```json
{
  "schema_version": 1,
  "event_id": "evt_1710000000_1",
  "correlation_id": "job_reflect_DEC001_xxx",
  "aggregate": { "type": "reflection", "id": "REF-000001" },
  "event_type": "ReflectionGenerated",
  "payload": {
    "reflection_id": "REF-000001",
    "decision_id": "DEC-000001",
    "outcome_id": "OUT-000001",
    "lessons": [{ "category": "assumption_error", "domain": "investment", "description": "技术突破 ≠ 商业采用", "severity": "high", "confidence": 0.9, "evidence_basis": ["OUT-001", "SIG-023"] }],
    "rules": [{ "rule_id": "RULE-001", "condition": { "domain": "investment", "trigger": "AI startup evaluation" }, "action": { "type": "require_validation", "instruction": "verify paid customer adoption" }, "confidence": 0.85 }]
  },
  "metadata": { "actor": "system", "source": "reflection_engine", "generator_version": "reflection-v1" },
  "occurred_at": 1710000000,
  "created_at": 1710000000
}
```

### R2 Artifact Schema

```json
{
  "schema_version": 1,
  "artifact_type": "reflection",
  "reflection_id": "REF-000001",
  "generator_version": "reflection-v1",
  "decision_snapshot": { "id": "DEC-000001", "title": "...", "thesis": "...", "confidence": 0.8, "evidence": [] },
  "outcome_snapshot": { "outcome_type": "observation", "observation": "..." },
  "analysis": { "result": "wrong", "confidence_calibration": "overestimated", "quality_score": 0.85 },
  "lessons": [{ "category": "assumption_error", "domain": "investment", "description": "...", "severity": "high", "evidence_basis": ["OUT-001"] }],
  "future_rules": [{ "rule_id": "RULE-001", "condition": {...}, "action": {...}, "confidence": 0.85 }],
  "created_at": 1710000000
}
```

### D1 Reflections 表

```sql
CREATE TABLE reflections (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    decision_id         INTEGER NOT NULL,
    outcome_id          INTEGER,
    status              TEXT NOT NULL DEFAULT 'pending',
    artifact_key        TEXT,
    result              TEXT,
    quality_score       REAL,
    generator_version   TEXT DEFAULT 'reflection-v1',
    lessons_count       INTEGER DEFAULT 0,
    rules_count         INTEGER DEFAULT 0,
    generated_by        TEXT DEFAULT 'system',
    retry_count         INTEGER DEFAULT 0,
    last_error          TEXT,
    started_at          INTEGER,
    lease_until         INTEGER,
    created_at          INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at          INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(decision_id)
);

CREATE INDEX idx_reflections_status ON reflections(status);
```

### Status 状态机

```
pending
  ↓
generating
  ↓ (success)        (failure)
generated            failed
  ↓ (retry)
generating (retry_count++)
```

Plus stale recovery: if `status='generating' AND updated_at < now - 3600` → set `status='failed'`

### Memory Candidate 预留

Sprint 5.4 不创建 Memory。Reflection Engine 在 `ReflectionGenerated` 事件处即止。
Sprint 5.5 Memory Engine 消费 `ReflectionGenerated` 事件，评估 promotion。

---

## Section 4: Generation Pipeline

### 流程

```
Trigger (API/Cron)
    ↓
ReflectionJob { decision_id, trigger, correlation_id }
    ↓
1. ContextBuilder → load Decision + Outcome + Evaluation + Signal evidence
    ↓
2. Completeness check → score < 0.4 → early ReflectionFailed
    ↓
3. Generator (LLM prompt → structured JSON)
    ↓
4. Validation → schema + grounding + quality
    ↓
5. Persister → D1 + outbox (transactional)
    ↓
6. Outbox worker → EventStore + R2 (async)
```

### Prompt Contract

System Prompt: "Strategic Decision Analyst" role — only infer from provided evidence, never invent external facts.

Input: DecisionSnapshot + OutcomeSnapshot + EvaluationSnapshot + EvidenceItems
Output: result, confidence_calibration, quality_score, lessons[].lessons, rules[].rules

### Validation

| Rule | Action |
|------|--------|
| `result ∈ {correct, wrong, mixed}` | fail on mismatch |
| `lessons ≥ 1` | fail |
| `description ≥ 20 chars` | fail |
| `confidence ∈ [0.0, 1.0]` | fail |
| `evidence_basis.length > 0` per lesson | warning (fail if empty) |
| `action.type + action.instruction != null` per rule | fail |

### Failure Handling

- LLM timeout / parse error → status=failed, retry_count++, last_error=log
- Validation fail → status=failed, retry_count++, last_error=log
- `retry_count >= 3` → stop retrying, require manual re-trigger

---

## Section 5: API & Cron Implementation

### 统一 Job 模型

```rust
pub struct ReflectionJob {
    pub decision_id: i64,
    pub trigger: ReflectionTrigger,
    pub correlation_id: String,
}
```

### API

```
POST /api/intelligence/decisions/:id/reflect
→ 202 Accepted
{
  "job_id": "job_reflect_DEC001_xxx",
  "decision_id": "DEC-001",
  "status": "pending"
}
```

MVP 返回 202（半异步）。API 创建 reflection row（status=pending）+ outbox。
Worker/Cron 消费 outbox 执行 `ReflectionEngine::execute(job)`。

### Cron

```rust
// In cron handler chain: ingestion → gc → signal → briefing → archive → reflection
reflection::process_pending_reflections(&env, now).await;
```

Batch size: 3 per cycle. Picks:
1. Completed decisions (>7d) without reflection
2. Failed reflections with retry_count < 3
3. Stale generating (lease expired)

### Concurrency

`UNIQUE(decision_id)` + `status='generating'` as optimistic lock.
`INSERT ... ON CONFLICT(decision_id) DO NOTHING RETURNING id` ensures single writer.

### Retry

```sql
UPDATE reflections SET status='generating', retry_count=retry_count+1
WHERE status='failed' AND retry_count < 3
```

### Lease

Set `lease_until = now + 600` when starting execution.
Stale recovery: `UPDATE ... WHERE status='generating' AND lease_until < now` → `failed`

---

## Section 6: Memory Promotion Interface

### Sprint 边界

| Sprint | 范围 |
|--------|------|
| **5.4 (当前)** | Reflection Engine — 生成 Reflection + `ReflectionGenerated` 事件 |
| **5.5 (未来)** | Memory Engine — 消费事件 → Promote → R2 memory/rules/ |

### Promotion Gate（Sprint 5.5 实施）

```
quality_score >= 0.7
AND outcome exists
AND evidence exists
AND (rules >= 1 OR lessons >= 1)
```

### Memory 类型（预留）

| memory_class | examples |
|---|---|
| decision_rule | 可执行规则：condition → action |
| domain_insight | 认知型：技术突破 ≠ 商业采用 |
| strategy_pattern | 战略模式：先验证后投资 |

### R2 Memory 目录（预留）

```
memory/
  reflections/REF-{id}.json       ← Sprint 5.4
  rules/MEM-{id}.json             ← Sprint 5.5
  insights/MEM-{id}.json          ← Sprint 5.5
  candidates/MEM-CAND-{id}.json   ← Sprint 5.5
```

---

## 验证方案

1. `cargo check --workspace` + `cargo test --workspace`
2. **API test**: POST /reflect → 202 + reflection row created
3. **Context test**: Decision without outcome → completeness score < 0.4 → ReflectionFailed
4. **Validation test**: Empty lessons → persisted as failed with `last_error`
5. **Cron test**: scan → pick eligible decisions → execute ReflectionEngine
6. **Durability test**: LLM success → D1 + outbox + EventStore consistent
7. **Retry test**: Failed reflection → cron retry → retry_count incremented
8. **Stale recovery test**: generating stuck > 1h → cron recovers to failed
