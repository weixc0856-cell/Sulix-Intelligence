<p align="center">
  <a href="README.md">🇬🇧 English</a> · <a href="README.zh-CN.md">🇨🇳 中文</a>
</p>

# Sulix Intelligence

> RSS Feed + AI Digest — 全量部署于 Cloudflare Workers 之上。

订阅 RSS/Atom 源，通过规则引擎对文章进行评分，经由 DeepSeek V4 Flash 摘要和打标，最终呈现为精选情报流。管线指标和可观测性通过 KV 逐周期记录。

## 架构

```
Cron 触发器（每 30 分钟）→ FETCH_QUEUE → 队列消费者
  → RSS 抓取 → D1 存储 → 规则引擎 → AI 管线 → Vectorize 索引
  → KV 管线指标 → /api/pipeline/status

HTTP (Worker Router) ←─ service binding ─→ Astro 前端 (Worker)
```

代码库正在向 **Ports & Adapters** 迁移（严格单向依赖）：

```
delivery (api + worker-entry)
      ↓ 只依赖 application + 领域类型 + worker
application（用例服务，泛型注入端口）
      ↓
domain（intelligence-domain / decision-engine / reasoning-framework / shared-kernel）
      ↑ 端口（Repository traits 定义在领域层）
infrastructure（D1 适配器，实现领域 trait）──→ store（仅作为 D1 数据访问库）
```

`store` 正在从 5800 行的上帝对象拆除为纯 D1 数据访问层。迁移状态与 P2–P7 / T6–T9 路线图见 `docs/superpowers/plans/`（详见 [迁移状态](#迁移状态)）。

## 认知管线

```
来源注册 → 内容策略 → 观察记录
     ↓
信号检测 → 信号聚类 → 评分
     ↓
主张提取
     ↓
决策引擎 → 结果追踪 → 反思学习
     ↓
置信度引擎（因子可解释）
     ↓
记忆引擎 → 上下文引擎
```

## Crate 一览

| Crate | 用途 |
|---|---|
| `api` | 73+ HTTP 路由（delivery 层）— 信号、决策、主张、置信度、来源、观察、合规、图谱、订阅源 CRUD、文章、策略 |
| `worker-entry` | Workers 入口：HTTP + Cron + Queue + 指标收集 |
| `application` | 用例服务（端口注入）— 雷达投影、决策图谱、语义搜索 |
| `intelligence-domain` | 情报域类型 + `Observation/Claim/Signal` 仓库端口 |
| `decision-engine` | 决策聚合、状态机 + `DecisionRepository` 端口 |
| `reasoning-framework` | 推理解释的 framework/applied-trace 类型 |
| `claim-engine` | 从信号提取主张 |
| `shared-kernel` | 跨情报 crate 的共享值对象/ID/事件 |
| `infrastructure` | 实现领域仓库 trait 的 D1 适配器（`decision/signal/claim/reflection/memory_repository.rs`） |
| `store` | D1 数据访问层（原上帝对象；正在收缩，`StoreBackend` `#[deprecated]` 带硬 TTL） |
| `intelligence/signal-engine` | 信号发现、聚类、评分、雷达投影、批量证据查询 |
| `intelligence/reflection-engine` | 决策反思生命周期 — 从结果生成结构化经验 |
| `memory-engine` | 长时记忆提升 — 评估、评分、归档记忆制品 |
| `context-engine` | 意图感知的上下文快照组装，用于 Agent 查询 |
| `agent-engine` | Agent 推理运行时 + 证据验证 |
| `model-runtime` | LLM 抽象（`Summarizer`/`HttpClient` trait + 测试用 `MockProvider`） |
| `content-governance` | 纯逻辑策略评估 — 按源层级控制存储/服务/嵌入/AI |
| `fetcher` | RSS/Atom 抓取 + SSRF 防护 + 全文提取（按源开启） |
| `rules` | 评分引擎（关键词匹配、来源过滤、AND/OR）— 纯逻辑 |
| `search` | D1 FTS5 关键词搜索 + 可选标签/分类过滤 |
| `entity` | 实体值对象 + 仓库 |
| `embedding` | Workers AI 嵌入向量（bge-large-en-v1.5） |
| `vectorize` | Cloudflare Vectorize 共享 wasm 绑定 |
| `events` | 事件契约（outbox 事件定义） |
| `event-store` | 事件溯源 — outbox-first 写入 → 异步归档至 R2 |
| `object-store` | 对象存储 trait + R2Store 实现 |
| `ai-pipeline` | LLM 摘要 + 标签归一化 |

## 关键设计决策

- **Cloudflare Workers**（非 VPS）— 单人运维成本可控，免费套餐，原生 D1/Queues/R2
- **D1 + FTS5** — CF 生态内唯一的结构化存储方案，触发器维护全文索引
- **Cloudflare Queues** — 每源隔离，内置重试，无超时风险
- **Astro server mode + service binding** — 每请求取最新数据，无需为新增文章重建
- **CQRS + 事件溯源** — 写（Repository trait）与读（QueryService trait）分离；事件仅追加、不可变
- **因子置信度** — 置信度 = 证据 × 来源可信 × 新鲜度 × 校准的几何平均，完全可解释
- **来源治理** — 每个 feed 对应一条 source 记录（层级、策略、许可、信任分），内容策略在摄取和服务两阶段强制执行
- **溯源链** — 每个智能制品携带 Source → Observation → Signal → Claim → Decision → Memory 的完整溯源
- **worker::Router**（非 Axum）— `worker::Env`/`D1Database` 不满足 `Send`/`Sync`，`worker::Router` 为此场景设计
- **StoreBackend trait** — `D1Store`（生产）与 `MemoryStore`（测试）通过 trait 互换；管线对任何后端泛型（正拆除为按域端口）

## 快速开始

```bash
# 需要 wasm32-unknown-unknown 目标
cargo check --workspace
cargo test --workspace              # 379 个单元测试（2026-09-06 实测）
cargo clippy --workspace -- -D warnings
cargo fmt --check

cargo install worker-build
cd crates/worker-entry
worker-build --release
npx wrangler deploy -c wrangler.toml
npx wrangler d1 migrations apply sulix-feed-db --remote
```

## API 端点

### 系统
| 端点 | 说明 |
|---|---|
| `GET /api/health` | 源/文章/Cron 统计 |
| `GET /api/dashboard` | 健康检查 + 每源统计 |
| `GET /api/pipeline/status` | 管线健康 + 执行耗时 |
| `GET /api/stats` | 分数分布 + 文章趋势 |
| `GET /api/tags` | 聚合标签云 |
| `GET /api/categories` | 分类列表 |
| `GET /api/intelligence/trust` | 信任中心 — 准确率、来源可信度 |

### 文章与订阅源
| 端点 | 说明 |
|---|---|
| `GET /api/articles/latest` | 最新文章 |
| `GET /api/articles/trending` | 高分文章 |
| `GET /api/articles/search` | FTS5 关键词 + 语义搜索 |
| `GET /api/articles/batch` | 批量获取 |
| `GET /api/articles/:id` | 文章详情（含溯源信息） |
| `GET /api/articles/:id/content` | 文章全文（受策略管控） |
| `GET/POST/PUT/DELETE /api/feeds` | 订阅源 CRUD |

### 情报
| 端点 | 说明 |
|---|---|
| `GET /api/intelligence/signals` | 今日信号摘要 |
| `GET /api/intelligence/radar` | 雷达仪表盘 |
| `GET /api/intelligence/signals/:id` | 信号详情 |
| `GET /api/intelligence/signals/:id/provenance` | 信号溯源链 |
| `GET /api/intelligence/briefing/today` | 每日情报简报 |
| `GET /api/intelligence/briefings` | 简报历史 |
| `GET /api/intelligence/entities` | 实体图谱 |
| `GET /api/intelligence/entities/:id/*` | 实体详情、文章、信号、关系、活跃度 |

### 决策智能
| 端点 | 说明 |
|---|---|
| `GET /api/intelligence/decisions` | 决策列表（?status=） |
| `POST /api/intelligence/signals/:id/decisions` | 为信号创建决策 |
| `GET /api/intelligence/decisions/stats` | 决策准确率 |
| `GET /api/intelligence/decisions/:id` | 决策详情 |
| `POST /api/intelligence/decisions/:id/status` | 更新决策状态 |
| `POST /api/intelligence/decisions/:id/reflect` | 触发 AI 反思 |
| `POST /api/intelligence/decisions/:id/outcomes` | 记录结果 |
| `POST /api/intelligence/decisions/:id/evaluations` | 记录评估 |
| `GET /api/intelligence/decisions/:id/timeline` | 合并决策时间线 |
| `GET /api/intelligence/decisions/:id/explanation` | 为什么系统相信这个判断 |
| `GET /api/projections/decision-graph` | 认知图谱投影 |
| `POST /api/projections/decision-graph/:id/expand` | 展开图谱节点 |

### 主张与置信度
| 端点 | 说明 |
|---|---|
| `GET /api/claims/:id` | 主张详情（含证据） |
| `GET /api/confidence/:type/:id` | 置信度演化时间线 |

### 治理
| 端点 | 说明 |
|---|---|
| `GET/POST/PUT/DELETE /api/sources` | 来源注册表 CRUD |
| `GET /api/observations` | 观察记录列表 |
| `GET /api/observations/:id/lineage` | 完整溯源链 |
| `POST /api/compliance/takedown` | 提交下架请求 |
| `GET /api/compliance/takedowns` | 下架请求列表（管理） |

### 内部
| 端点 | 说明 |
|---|---|
| `POST /api/internal/agent/run` | Agent 推理引擎 |
| `POST /api/internal/context` | 上下文快照组装 |
| `POST /api/strategies/preview` | 预览信号策略影响 |
| `GET/POST/PUT/DELETE /api/rules` | 过滤/评分规则 CRUD |

这些端点的前端侧权威契约（DTO、分页、null-safety）位于前端仓库 `docs/api-contract.md`（§11/§12 Explicit API Contract）。后端在 P2–P5 期间按其 audit 自身 DTO。

## CI/CD

推送 `master` → GitHub Actions。三道独立门禁：

1. **`lint.yml`**（PR）：`cargo-deny` bans（store/vectorize/embedding/event-store/object-store 禁止作为 infra/delivery 之外的新增依赖）→ 分层依赖脚本 → `cargo fmt --check` → `cargo clippy -- -D warnings` → **wasm32 检查**（`cargo check --target wasm32-unknown-unknown`）→ `cargo test --workspace`
2. **`coverage.yml`**（PR）：`cargo-llvm-cov` 覆盖 14-crate 纯逻辑 + 应用层集合，**`--fail-under-lines 70` 硬门禁**（当前 73.84%），上传 lcov 报告
3. **`deploy.yml`**（push 至 master）：wasm 检查 → `worker-build --release` → `npx wrangler deploy` → 冒烟测试（health + semantic search）

Secrets: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`

## 迁移状态

Sprint 6.5 去耦（Store 上帝对象拆除）**已完成**（2026-09-06）。`application → store` 正常依赖边归零，
`GRANDFATHERED` 空表且全硬禁。新 crate：`domain`（infra-free 端口 + DTO + `StoreError`）与 `composition`
（wiring-only，仅 `ProductionAppServices = AppServices<D1Store>`）。

**已完成：** P1（依赖 bans + 分层依赖门禁，`GRANDFATHERED` = 0）· P2（Reflection/Memory/Signal/Context
域仓库端口）· P3 部分（ai-pipeline 脱离 store；signal-engine + context-engine 经 ports 脱离
store/vectorize/event-store）· P4（`StoreBackend` body → 0，保留为**空 composite** 供 worker-entry
`DecisionService` 合成，GATED 写方法迁 `DecisionWriteStore`）· P5（application 唯一用例入口；
Source/Entity 上收；composition-root 注入）· Phase 2 + C1–C7（`api → concrete-infra` = 0；
application 改指 infra-free `domain`；store → dev-dep）· P7（cargo-metadata 架构守卫入 CI）· T1（基线修复）·
T2（infrastructure 适配器测试）· T3（shared-kernel/events 契约测试）· T4（llvm-cov + 70% 门禁）·
T5（PR wasm 门禁）· T10（基线追踪）

**待办：** GATED **decision vertical**（补全 decision-engine 域 → 切除最后 4 条 `DecisionWriteStore`
写方法 + 空 `StoreBackend` composite）→ **P6 范围裁决**（intelligence-domain 存续待决议 —— frozen arch
v2 §P6 与现行用法矛盾）→ 测试 T6（应用层用例测试）、T7（去耦每 commit 硬约束）、T8（跨域集成：
observe→claim→signal→decision→reflection）、T9（delivery 层测试）

状态与路线图：`docs/status-roadmap-2026-09-06.md`。计划文档：`docs/superpowers/plans/2026-08-21-architecture-decoupling-plan.md`（P1–P7）、`docs/superpowers/plans/2026-08-21-testing-plan.md`（T1–T10）与 `docs/superpowers/plans/2026-09-05-decoupling-advance.md`。

## 前端

[intel.getsulix.com](https://intel.getsulix.com) — Astro 5 前端，以 Cloudflare Worker + service binding 部署。功能包括情报雷达、信号调查、决策追踪、信任中心、来源溯源、语义搜索、深色模式、订阅源管理和认知图谱。

仓库：[weixc0856-cell/Intel-Web](https://github.com/weixc0856-cell/Intel-Web)

## 许可证

MIT
