# Sulix Storage Architecture v1.1 — Final Freeze Contract

**Status:** FROZEN
**Date:** 2026-07-26
**Supersedes:** Storage Boundary Contract v1 (Sprint 5.9)

## D1 定位
Transactional State Store + Query Projection Layer。Sulix 的业务数据库，负责结构化业务状态、当前状态查询和高价值索引。

D1 ≠ EventStore 的缓存。Entity、Signal、Decision、Outcome 本身就是业务状态，长期存在于 D1。

## Storage Boundary

| 数据 | 存储位置 | 理由 |
|------|---------|------|
| Entity structured data | D1 | 核心业务实体状态 |
| Article metadata | D1 | title, summary, status, source |
| Article content | R2 | 大文本、原始内容 |
| Signal state | D1 | score, trend, status |
| Decision state | D1 | title, hypothesis, confidence |
| Outcome state | D1 | 当前结果状态 |
| Brief 全文 | R2 | 大型生成内容 |
| Agent Context Snapshot | R2 | 大型状态快照 |
| Reflection artifact | R2 | 长文本分析结果 |
| AI output | R2 | LLM 原始生成结果 |
| Event payload ≤8KB | EventStore | 轻量领域事件 |
| Event payload >8KB | R2 + artifact_key | 大事件负载 |
| Embedding | VectorStore | 语义检索索引 |
| **Artifact metadata** | **D1** | **查询和生命周期管理（artifacts 表）** |

## Artifact Pointer Contract
所有跨层大对象引用统一使用 `artifact_key`，格式 `{domain}/{id}/{version}.json`。

D1 保存 `artifact_key TEXT`，不保存 `content TEXT` 或 `large_json TEXT`。

## Quota Target（Beta）

| 指标 | 目标 | 安全余量 | 说明 |
|------|------|---------|------|
| D1 Writes/month | <50k | >50% | 所有新增功能必须评估 monthly row operations impact |
| D1 Reads/month | <2M | >60% | Quota Target 是架构预算，不是硬上限 |
| D1 Storage | <500MB | 充裕 | — |
| R2 Storage | 持续增长 | 低成本扩展 | Artifact 存储 |

## Core Rules

### Rule 1 — D1 是业务数据库
D1 ≠ projection cache。Entity、Signal、Decision、Outcome 等结构化业务状态天然属于 D1。

### Rule 2 — 控制 Write Amplification
目标：**1 个业务状态变化 ≤ 5 D1 writes**。Event 不一定对应 D1 写（可能仅 EventStore append + R2 artifact）。禁止产生 N² 关系写入、重复事件、遗留双写。

### Rule 3 — 周期任务幂等
所有 cron / agent pipeline 必须具备 input fingerprint → compare → changed? → persist / skip。No-op execution 不产生 persistence writes。

### Rule 4 — API Query Contract
禁止 `SELECT *`、禁止无限列表读取。必须 pagination / cursor / limit / projection query。Large aggregation 必须使用 precomputed projection，禁止实时 JOIN 多表拼装。

### Rule 5 — EventStore 单一历史来源
EventStore 是 **domain history source of truth**（领域变化历史），不是全部数据的 source of truth。当前状态（如 Decision.status）仍以 D1 为准。禁止 Domain Event 双写。

## Layer Responsibility Diagram

```
                    User/API
                       |
               Application Layer
                       |
       +---------------+---------------+
       |               |               |
      D1          EventStore          R2
 Current State    Domain History    Artifacts
       |               |
       +---------------+---------------+
                       |
                  VectorStore
              Semantic Retrieval
```

## Future Sprint Review Rule
所有新功能设计只需判断：
1. **当前状态 / 结构化业务数据？** → **D1**
2. **大文本 / AI 生成物 / Snapshot？** → **R2**
3. **领域变化历史？** → **EventStore**
4. **语义召回？** → **VectorStore**
