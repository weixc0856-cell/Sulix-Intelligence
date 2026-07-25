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
| D1 | Reflection state + index（reflections 表 + outbox 消息） |
| R2 | 完整 Reflection Artifact（memory/reflections/REF-{id}.json） |
| EventStore | 轻量 Reflection lifecycle events（metadata + pointer，不含全文） |

### Outbox 职责分离

MVC 只用一个 `object_outbox` 表，但代码层抽象两个语义：

- **Task Outbox**: domain job dispatch（ReflectionRequested → ReflectionEngine）
- **Archive Outbox**: artifact archive（ReflectionGenerated → R2）

代码通过 `outbox.object_type` 区分：`event:*` vs `archive:*`。

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
      generator.rs                   ← LLM prompt + parse (trait + impl)
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
    jobs/reflection.rs               ← Cron 扫描 + 触发 ReflectionEngine
    jobs/mod.rs                      ← 注册
    runtime/cron.rs                  ← 加 process_pending

migrations/
  0024_reflection_engine.sql         ← reflections 表
```

### ReflectionEngine（领域服务）

```rust
pub struct ReflectionEngine<R, E, G>
where
    R: ReflectionRepository,
    E: EventStore,
    G: ReflectionGenerator,
{
    repository: R,
    event_store: E,
    generator: G,
}
```

各组件职责：

- **ReflectionContextBuilder** (`context.rs`): 加载 Decision + Thesis + Evidence + Outcome → `ReflectionContext`
- **ReflectionGenerator trait** (`generator.rs`): 调用 LLM → 结构化 Reflection JSON 草稿
- **Validation Layer** (`validation.rs`): Schema + grounding + quality 验证
- **ReflectionPersister**: D1 state + Task/Event/Archive outbox（不直接写 R2）

### ReflectionGenerator trait

```rust
#[async_trait(?Send)]
pub trait ReflectionGenerator {
    async fn reflect(&self, context: &ReflectionContext) -> Result<ReflectionDraft, String>;
}
```

不绑定 HttpSummarizer。Future 可接 DeepSeek / OpenRouter / Cloudflare AI / Local。

### 不分叉原则

Domain service 永远不直接写 artifact storage。所有 durable projection 通过 state + outbox 驱动：

```
D1 transaction
  ├── INSERT reflections (status=reflection_requested)
  ├── task_outbox (type: "ReflectionRequested")
  ├── event_outbox (type: "ReflectionGenerated")     ← 事件已产生时
  └── archive_outbox (type: "ReflectionArtifact")     ← 事件已产生时
COMMIT
  ↓
archive worker → EventStore (事件) → R2 (artifact)
```

### Three-semantic outbox（同一张 object_outbox 表，object_type 区分）

| outbox object_type | 语义 | 消费者 |
|---|---|---|
| `task:reflection` | 领域任务调度 | Cron Worker 消费 |
| `event:reflection` | 领域事件 | Event Dispatcher → EventStore |
| `archive:reflection` | 持久化投影 | Artifact Worker → R2 |

---

## Section 3: Data Model & Event Schema

### EventEnvelope（轻量）

ReflectionGenerated 事件 payload 只保存 metadata + pointer，不保存全文：

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
    "artifact_key": "memory/reflections/REF-000001.json",
    "quality_score": 0.85,
    "lesson_count": 2,
    "rule_count": 1
  },
  "metadata": {
    "actor": "system",
    "source": "reflection_engine",
    "generator_version": "reflection-v1"
  },
  "occurred_at": 1710000000,
  "created_at": 1710000000
}
```

### R2 Artifact Schema（完整内容）

`memory/reflections/REF-000001.json`:

```json
{
  "schema_version": 1,
  "artifact_type": "reflection",
  "reflection_id": "REF-000001",
  "generator_version": "reflection-v1",
  "source_chain": {
    "decision_id": "DEC-000001",
    "outcome_id": "OUT-000001",
    "signals": ["SIG-001", "SIG-002"]
  },
  "decision_snapshot": {
    "id": "DEC-000001",
    "title": "投资 AI Agent 公司",
    "decision_type": "investment"
  },
  "thesis_snapshot": {
    "hypothesis": "AI Agent 市场将在 12 个月内快速增长",
    "assumptions": ["技术突破会迅速转化为商业产品"],
    "initial_confidence": 0.8
  },
  "outcome_snapshot": {
    "outcome_type": "observation",
    "observation": "增长低于预期，客户留存仅 30%"
  },
  "analysis": {
    "result": "wrong",
    "confidence_calibration": "overestimated",
    "quality_score": 0.85
  },
  "lessons": [
    {
      "category": "assumption_error",
      "domain": "investment",
      "description": "技术突破 ≠ 商业采用，低估了客户教育成本",
      "severity": "high",
      "confidence": 0.9,
      "evidence_basis": ["OUT-001", "SIG-023"]
    }
  ],
  "future_rules": [
    {
      "rule_id": "RULE-001",
      "condition": { "domain": "investment", "trigger": "AI startup evaluation" },
      "action": { "type": "require_validation", "instruction": "verify paid customer adoption" },
      "confidence": 0.85
    }
  ],
  "created_at": 1710000000
}
```

### D1 Reflections 表

```sql
CREATE TABLE reflections (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    decision_id         INTEGER NOT NULL,
    outcome_id          INTEGER,
    job_id              TEXT UNIQUE,              -- "job_reflect_DEC001_xxx"
    status              TEXT NOT NULL DEFAULT 'pending',
    artifact_key        TEXT,
    result              TEXT,                     -- correct | wrong | mixed | unknown
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
CREATE UNIQUE INDEX idx_reflections_job_id ON reflections(job_id);
```

### Status 状态机

```
pending (requested)
    ↓
generating
    ↓ (success)        (failure)
generated              failed
    ↓ (retry)
generating (retry_count++)
```

| D1 status | 对应 Event | 触发 |
|---|---|---|
| pending | ReflectionRequested | Job 创建 |
| generating | ReflectionStarted | claim job 成功 |
| generated | ReflectionGenerated | LLM + validation 成功 |
| failed | ReflectionFailed | 任何不可恢复错误 |

Stale recovery: `status='generating' AND lease_until < now` → `status='failed'`, `last_error='lease_expired'`

---

## Section 4: Generation Pipeline

### 流程

```
Trigger (API/Cron)
    ↓
ReflectionJob { decision_id, trigger, correlation_id }
    ↓
1. ContextBuilder → load Decision + Thesis + Outcome + Evaluation + Signal evidence
    ↓
2. Completeness check
       score = decision*0.3 + thesis*0.2 + outcome*0.3 + evidence*0.2
       score < 0.4 → early ReflectionFailed { reason: "insufficient_context" }
    ↓
3. Generator (LLM prompt → structured JSON draft)
    ↓
4. Validation → schema + grounding + quality
    ↓
5. Persister (D1 transaction):
       ├── INSERT reflections (status=generated)
       ├── event_outbox (type=ReflectionGenerated, lightweight)
       └── archive_outbox (type=ReflectionArtifact)
    ↓
6. Outbox worker → EventStore archive → R2 artifact
```

### ReflectionContext

```rust
pub struct ReflectionContext {
    pub decision: DecisionSnapshot,
    pub thesis: ThesisSnapshot,
    pub outcome: Option<OutcomeSnapshot>,
    pub evaluations: Vec<EvaluationSnapshot>,
    pub evidence: Vec<EvidenceItem>,
    pub completeness_score: f64,
}

pub struct DecisionSnapshot {
    pub id: i64,
    pub title: String,
    pub decision_type: String,
}

pub struct ThesisSnapshot {
    pub hypothesis: String,
    pub assumptions: Vec<String>,
    pub initial_confidence: f64,
}

pub struct EvidenceItem {
    pub source: String,           // signal_id / article_url
    pub summary: String,
    pub relevance_score: f64,
    pub captured_at: i64,
}
```

### Context Completeness Score

```
score = decision_exists * 0.3
      + thesis_exists * 0.2
      + outcome_exists * 0.3
      + evidence_exists * 0.2
```

每个因子 0 或 1。总分 [0.0, 1.0]。`< 0.4` 跳过 LLM，直接 ReflectionFailed。

### Prompt Contract

System Prompt: "Strategic Decision Analyst" — only infer from provided evidence, never invent external facts. If insufficient, mark uncertainty.

Input: DecisionSnapshot + ThesisSnapshot + OutcomeSnapshot + EvaluationSnapshots + EvidenceItems

Output: result, confidence_calibration, quality_score, lessons[], rules[]

### Validation

```rust
pub struct ValidationResult {
    pub valid: bool,
    pub quality_score: f64,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}
```

| Rule | Action |
|------|--------|
| `result ∈ {correct, wrong, mixed}` | fail |
| `lessons ≥ 1` | fail |
| `description ≥ 20 chars` | fail |
| `confidence ∈ [0.0, 1.0]` | fail |
| `evidence_basis.length > 0` per lesson | **fail**（必须可追溯） |
| `action.type + action.instruction != null` per rule | fail |

### Failure Handling

- LLM timeout / parse error → `status=failed, retry_count++, last_error=log`
- Validation fail → `status=failed, retry_count++, last_error=log`
- Insufficient context (completeness < 0.4) → `status=failed, retry_count=3`（不重试）
- `retry_count >= 3` → stop retrying, require manual re-trigger via API

---

## Section 5: API & Cron Implementation

### 统一 Job 模型

```rust
pub struct ReflectionJob {
    pub decision_id: i64,
    pub trigger: ReflectionTrigger,
    pub correlation_id: String,
}

pub enum ReflectionTrigger { Api, Cron }
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

实现：创建 D1 reflection row (status=pending, job_id=...) + task outbox。Worker/Cron 消费执行。

### Cron

```rust
// In cron handler: ingestion → gc → signal → briefing → archive → reflection
reflection::process_pending_reflections(&env, now).await;
```

Batch size: **3 per cycle**. Picks:

1. Completed decisions (>7d) without reflection
2. Failed reflections with retry_count < 3
3. Stale generating (lease_until < now)

### Concurrency Control

`UNIQUE(decision_id)` + `status='generating'` as optimistic lock.

```sql
INSERT INTO reflections (decision_id, status)
VALUES (?1, 'generating')
ON CONFLICT(decision_id) DO NOTHING
RETURNING id;
```

Only one writer succeeds. Failed insert → skip.

### Retry

```sql
UPDATE reflections
SET status='generating', retry_count=retry_count+1
WHERE status='failed' AND retry_count < 3
```

### Lease

Set `lease_until = now + 900` (15 min) when starting execution.
Stale recovery: cron picks up `status='generating' AND lease_until < now`, sets `status='failed'`, `last_error='lease_expired'`

---

## Section 6: Memory Promotion Interface

### Sprint 边界

| Sprint | 范围 |
|--------|------|
| **5.4 (当前)** | Reflection Engine — 生成 Reflection + `ReflectionGenerated` 轻量事件 |
| **5.5 (未来)** | Memory Engine — 消费事件 → Promote → R2 memory/rules/ |

### EventStore 消费模型

```json
{ "consumer": "memory-engine", "last_event_id": "evt_1710000000_1" }
```

EventStore 天然支持多消费者。Memory Engine 在 Sprint 5.5 从 EventStore 消费。

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
2. **API test**: POST /reflect → 202 + reflection row created with job_id
3. **Idempotency test**: 同一 decision 重复 POST → 409 / already exists
4. **Completion check test**: Decision without outcome → completeness 0.3 → < 0.4 → ReflectionFailed
5. **Validation test**: Empty lessons → failed with `last_error`
6. **Cron test**: scan → pick eligible decisions → execute ReflectionEngine
7. **Durability test**: LLM success → D1 + outbox consistent (no orphan artifact)
8. **Retry test**: Failed reflection (retry_count=1) → cron retry → increment
9. **Stale recovery test**: generating + lease expired → cron recovers to failed
10. **Event replay test**: ReflectionGenerated event → rebuild reflection index
