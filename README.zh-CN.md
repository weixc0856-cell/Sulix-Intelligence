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

### Crate 依赖图

```
worker-entry → api → store → worker (D1, Queues, Router)
            → fetcher → worker, feed-rs
            → rules（纯逻辑 — 无 worker 依赖）
            → ai-pipeline → store（通过 StoreBackend trait）, Summarizer trait
            → vectorize（共享 wasm 绑定）
api → store, search, rules, embedding, vectorize
store → worker (D1Database)
```

## Crate 一览

| Crate | 用途 |
|---|---|
| `store` | D1 访问层（feeds、articles、CRUD、健康检查）+ `StoreBackend` trait + `MemoryStore` 测试实现 |
| `vectorize` | 共享 `#[wasm_bindgen]` 绑定 — Cloudflare Vectorize（upsert + query + delete） |
| `fetcher` | RSS/Atom 抓取 + SSRF 防护 + 全文提取（按源开启）+ AbortSignal 超时 |
| `rules` | 评分引擎（关键词匹配、来源过滤、AND/OR）— 纯逻辑，单元测试覆盖 |
| `ai-pipeline` | `Summarizer` / `HttpClient` trait + `HttpSummarizer`（兼容 OpenAI 协议）+ 标签归一化 |
| `search` | D1 FTS5 关键词搜索 + 可选标签/分类过滤 |
| `embedding` | Workers AI 嵌入向量（bge-large-en-v1.5） |
| `api` | HTTP 路由 — 健康检查、仪表盘、标签、feeds CRUD、文章、策略、管线状态 |
| `worker-entry` | `#[event(fetch/scheduled/queue)]` — Workers 入口 + 管线指标收集 |

## 关键设计决策

- **Cloudflare Workers**（非 VPS）— 单人运维成本可控，免费套餐，原生 D1/Queues/R2
- **D1 + FTS5**（非 Postgres/Meilisearch）— CF 生态内唯一的结构化存储方案，通过触发器维护全文索引
- **Cloudflare Queues**（非同步 cron 循环）— 每源隔离，内置重试，不存在超时风险
- **Astro 服务端渲染 + service binding**（非静态站点）— 每次请求获取最新数据，无需重建即可展示新文章
- **worker::Router**（非 Axum）— `worker::Env`/`D1Database` 不支持 `Send`/`Sync`；`worker::Router` 专为此场景设计
- **StoreBackend trait** — `D1Store`（生产环境）与 `MemoryStore`（测试环境）通过 trait 互换；管线对后端实现无感
- **SSRF 防护** — 抓取器拦截 IP 字面量与本地回环别名 URL；DNS 重绑定为已知局限

## 管线流程

```
crates/fetcher/          — RSS/Atom 抓取 → 全文提取（可选）→ Article
    ↓
crates/store/            — D1 去重 + 持久化
    ↓
crates/rules/            — 规则评分（关键词、来源、AND/OR）
    ↓
crates/ai-pipeline/      — LLM 摘要 + 标签归一化
    ↓
crates/embedding/ + crates/vectorize/ — 生成向量 → 存入 Vectorize 索引
    ↓
KV 管线指标               — 每周期执行时间、文章数、LLM 调用次数
```

## 快速开始

```bash
# 后端（需要 wasm32-unknown-unknown 目标）
cargo check --workspace
cargo test --workspace              # 100+ 单元测试
cargo clippy --workspace -- -D warnings
cargo fmt --check

cargo install worker-build          # 首次安装
cd crates/worker-entry
worker-build --release
npx wrangler deploy -c wrangler.toml
```

## API 端点

| 端点 | 说明 |
|---|---|
| `GET /api/health` | 源/文章/Cron 统计 |
| `GET /api/dashboard` | 健康检查 + 每源统计 |
| `GET /api/pipeline/status` | 管线健康 + 执行耗时 |
| `GET /api/tags` | 聚合标签云（含计数） |
| `GET/POST /api/feeds` | 列出 / 创建订阅源 |
| `GET/PUT/DELETE /api/feeds/:id` | 读取 / 更新 / 软删除 |
| `GET /api/articles/latest` | 最新文章（?tag=, ?limit=） |
| `GET /api/articles/trending` | 高分文章（score > 0） |
| `GET /api/articles/search?q=` | FTS5 关键词 + 语义搜索 |
| `GET /api/articles/:id` | 文章详情 |
| `GET /api/articles/:id/content` | 文章全文（来自 R2） |
| `GET /api/articles/:id/related` | 按共享标签推荐相关文章 |
| `GET /api/articles/:id/adjacent` | 上一篇 / 下一篇 |
| `GET/POST/PUT/DELETE /api/rules` | 过滤/评分规则 CRUD |
| `POST /api/strategies/preview` | 预览策略影响 |
| `POST /api/admin/rebuild-embeddings` | 批量重建嵌入向量 |

## CI/CD

推送至 `master` 分支 → GitHub Actions：
1. `cargo clippy --workspace -D warnings`
2. `cargo test --workspace`
3. `worker-build --release`
4. `wrangler deploy`
5. 冒烟测试（健康检查 + 语义搜索）

所需 Secrets：`CLOUDFLARE_API_TOKEN`、`CLOUDFLARE_ACCOUNT_ID`

## 前端

[intel.getsulix.com](https://intel.getsulix.com) — Astro 5 前端，以 Cloudflare Worker 形式部署，通过 service binding 连接后端 API。功能包括语义搜索、深色模式、标签云、热门文章、订阅源管理、信号策略与书签。

前端仓库：[weixc0856-cell/Intel-Web](https://github.com/weixc0856-cell/Intel-Web)

## 项目结构

```
D:\Project\Sulix Intelligence（Rust 工作区 — 后端）
├── Cargo.toml               ← 工作区根配置（9 个成员 crate）
├── migrations/
│   └── 0001_init.sql        ← D1 数据库模式（feeds, articles, filter_rules）
├── crates/
│   ├── store/               ← D1 访问层 + StoreBackend trait + MemoryStore
│   ├── fetcher/             ← RSS/Atom 抓取 + SSRF 防护 + AbortSignal 超时
│   ├── rules/               ← 过滤/评分引擎（纯逻辑，单元测试覆盖）
│   ├── ai-pipeline/         ← AI 摘要 trait + HttpSummarizer + 标签归一化
│   ├── search/              ← FTS5 搜索抽象 + WHERE 条件构建器（已测试）
│   ├── embedding/           ← Workers AI 嵌入向量（bge-large-en-v1.5）
│   ├── vectorize/           ← Vectorize 共享 wasm 绑定（upsert/query/delete）
│   ├── api/                 ← HTTP 路由（worker::Router）
│   └── worker-entry/        ← Workers 入口（HTTP + Cron + Queue + 指标）

D:\Project\intel-web（Astro — 前端）
├── astro.config.mjs         ← @astrojs/cloudflare, server 模式
├── tailwind.config.mjs      ← "Informed Modernity" 设计系统
├── wrangler.toml             ← Worker 配置，指向 API Worker 的 service binding
└── src/
    ├── pages/               ← 17 个路由页面 + API 代理
    ├── components/          ← 15 个 Astro 组件
    ├── layouts/             ← Base → Marketing / Reader 布局层级
    ├── lib/api/             ← 按领域拆分的 API 客户端
    └── styles/              ← Tailwind + 自定义实用类
```

## 许可证

MIT
