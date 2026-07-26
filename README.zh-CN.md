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
| `store` | D1 访问层 + 22 个领域 trait + `StoreBackend` + `MemoryStore` 测试实现 |
| `intelligence/signal-engine` | 信号发现、聚类、评分、雷达投影、批量证据查询 |
| `intelligence/reflection-engine` | 决策反思生命周期 — 从结果生成结构化经验 |
| `memory-engine` | 长时记忆提升 — 评估、评分、归档记忆制品 |
| `context-engine` | 意图感知的上下文快照组装，用于 Agent 查询 |
| `agent-engine` | Agent 推理运行时 + 证据验证 |
| `content-governance` | 纯逻辑策略评估 — 按源层级控制存储/服务/嵌入/AI |
| `fetcher` | RSS/Atom 抓取 + SSRF 防护 + 全文提取（按源开启） |
| `rules` | 评分引擎（关键词匹配、来源过滤、AND/OR）— 纯逻辑 |
| `ai-pipeline` | LLM 摘要 + 标签归一化 |
| `search` | D1 FTS5 关键词搜索 + 可选标签/分类过滤 |
| `embedding` | Workers AI 嵌入向量（bge-large-en-v1.5） |
| `vectorize` | Cloudflare Vectorize 共享 wasm 绑定 |
| `api` | 73+ HTTP 路由 — 信号、决策、主张、置信度、来源、观察、合规、图谱 |
| `event-store` | 事件溯源 — outbox-first 写入 → 异步归档至 R2 |
| `object-store` | 对象存储 trait + R2Store 实现 |
| `application` | 雷达投影、决策图谱、语义搜索 |
| `worker-entry` | Workers 入口：HTTP + Cron + Queue + 指标收集 |

## 关键设计决策

- **Cloudflare Workers**（非 VPS）— 单人运维成本可控，免费套餐，原生 D1/Queues/R2
- **D1 + FTS5** — CF 生态内唯一的结构化存储方案，触发器维护全文索引
- **Cloudflare Queues** — 每源隔离，内置重试，无超时风险
- **CQRS + 事件溯源** — 写（Repository trait）与读（QueryService trait）分离；事件仅追加、不可变
- **因子置信度** — 置信度 = 证据 × 来源可信 × 新鲜度 × 校准的几何平均，完全可解释
- **来源治理** — 每个 feed 对应一条 source 记录（层级、策略、许可、信任分），内容策略在摄取和服务两阶段强制执行
- **溯源链** — 每个智能制品携带 Source → Observation → Signal → Claim → Decision → Memory 的完整溯源
- **StoreBackend trait** — `D1Store`（生产）与 `MemoryStore`（测试）通过 trait 互换

## 快速开始

```bash
# 需要 wasm32-unknown-unknown 目标
cargo check --workspace
cargo test --workspace              # 148+ 单元测试
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
| `GET /api/intelligence/entities` | 实体图谱 |
| `GET /api/intelligence/entities/:id/*` | 实体详情、文章、信号、关系 |

### 决策智能
| 端点 | 说明 |
|---|---|
| `GET/POST /api/intelligence/decisions` | 决策列表/创建 |
| `GET /api/intelligence/decisions/stats` | 决策准确率 |
| `GET /api/intelligence/decisions/:id` | 决策详情 |
| `GET /api/intelligence/decisions/:id/explanation` | 为什么系统相信这个判断 |
| `GET /api/intelligence/decisions/:id/timeline` | 决策时间线 |
| `GET /api/projections/decision-graph` | 认知图谱 |

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

## 项目结构

```
crates/
├── store/              D1 访问层 + 领域 trait
├── fetcher/            RSS 抓取 + SSRF
├── rules/              评分引擎
├── ai-pipeline/        LLM 摘要
├── search/             FTS5 搜索
├── embedding/          嵌入向量
├── vectorize/          Vectorize 绑定
├── content-governance/ 内容策略（纯逻辑）
├── api/                73+ HTTP 路由
├── intelligence/
│   ├── signal-engine/  信号引擎
│   └── reflection-engine/ 反思引擎
├── memory-engine/      记忆引擎
├── context-engine/     上下文引擎
├── agent-engine/       Agent 推理
├── event-store/       事件溯源
├── object-store/       R2 对象存储
├── application/       雷达/图谱/搜索
└── worker-entry/      Workers 入口
```

## CI/CD

推送 `master` → GitHub Actions：
1. `cargo clippy --workspace -D warnings`
2. `cargo test --workspace`
3. `worker-build --release`
4. `npx wrangler deploy`

## 前端

[intel.getsulix.com](https://intel.getsulix.com) — Astro 5 前端，以 Cloudflare Worker 部署。功能包括情报雷达、信号调查、决策追踪、信任中心、来源溯源、语义搜索、深色模式、订阅源管理和认知图谱。

## 许可证

MIT
