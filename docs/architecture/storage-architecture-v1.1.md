# Sulix Storage Architecture v1.1 — Final Freeze Contract

**Status:** FROZEN
**Date:** 2026-07-26
**Supersedes:** Storage Boundary Contract v1 (Sprint 5.9)

## D1 定位
Transactional State Store + Query Projection Layer。Sulix 的业务数据库，保存结构化、可查询、事务性数据。同时通过架构设计控制 Cloudflare D1 Monthly Row Operations 的读写放大。

## Storage Boundary

| 数据 | 存储位置 | 理由 |
|------|---------|------|
| Entity structured data | D1 | 核心业务实体状态 |
| Article metadata | D1 | title, summary, status, source, timestamps |
| Article content | R2 | 大文本、原始内容 |
| Signal state | D1 | score, trend, status, current assessment |
| Decision state | D1 | title, hypothesis, confidence, status |
| Outcome state | D1 | 当前结果状态、评价指标 |
| Brief 全文 | R2 | 大型生成内容 |
| Agent Context Snapshot | R2 | 大型状态快照 |
| Reflection artifact | R2 | 长文本分析结果 |
| AI output | R2 | LLM 原始生成结果 |
| Event payload ≤8KB | EventStore | 轻量领域事件 |
| Event payload >8KB | R2 + artifact_key | 大事件负载 |
| Embedding | VectorStore | 语义检索索引 |

## Artifact Pointer Contract
所有跨层大对象引用统一使用 `artifact_key`，格式 `{domain}/{id}/{version}.json`。

D1 保存 `artifact_key TEXT`，不保存 `content TEXT` 或 `large_json TEXT`。

## Quota Target（Beta）

| 指标 | 目标 | 安全余量 | 说明 |
|------|------|---------|------|
| D1 Writes/month | <50k | >50% | 保留 migration/replay/backfill 空间 |
| D1 Reads/month | <2M | >60% | 非逼近上限，保留增长空间 |
| D1 Storage | <500MB | 充裕 | — |
| R2 Storage | 持续增长 | 低成本扩展 | Artifact 存储 |

## Core Rules

### Rule 1 — D1 是业务数据库
D1 ≠ projection cache。Entity、Signal、Decision、Outcome 等结构化业务状态天然属于 D1。

### Rule 2 — 控制 Write Amplification
目标：1 Business Event ≤ 5 D1 writes。禁止一次业务变化产生 N² 关系写入、重复事件、遗留双写。

### Rule 3 — 周期任务幂等
所有 cron / agent pipeline 必须具备 input fingerprint → compare → changed? → persist / skip。

### Rule 4 — API Query Contract
禁止 `SELECT *`、禁止无限列表读取。必须 pagination / cursor / limit / projection query。

### Rule 5 — EventStore 单一历史来源
禁止 Domain Event 双写（EventStore + legacy_events_table）。统一 EventStore → Projection → D1。

## Layer Responsibility Diagram

```
                    User/API
                       |
               Application Layer
                       |
       +---------------+---------------+
       |               |               |
      D1          EventStore          R2
Transactional     Domain History    Artifacts
 State            Events            Content
       |               |
       +---------------+---------------+
                       |
                  VectorStore
              Semantic Retrieval
```

## Future Sprint Review Rule
所有新功能设计只需判断：
1. 这是结构化状态？→ **D1**
2. 这是大文本 / AI 产物 / Snapshot？→ **R2**
3. 这是状态变化历史？→ **EventStore**
4. 这是语义检索需求？→ **VectorStore**
