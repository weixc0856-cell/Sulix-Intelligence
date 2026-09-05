---
name: backend-dev-guide
description: Backend Rust workspace structure, commands, and design decisions
metadata:
  type: reference
---

# Sulix Intelligence V2 — Claude Dev Guide

## Architecture

```
Cron Trigger (every 30 min) → FETCH_QUEUE → Queue Consumer
  → RSS Fetch → D1 Store → Rules Engine → AI Pipeline → Vectorize Index
  → KV Pipeline Metrics → /api/pipeline/status

HTTP (Worker Router) ←─ service binding ─→ Astro Frontend (Worker)
```

Sulix is a curated RSS Feed + AI Digest product, deployed entirely on Cloudflare Workers.

## Project Structure

> **Workspace members**: the backend is a Rust workspace with **27 member crates**.
> The root [`Cargo.toml`](./Cargo.toml) `[workspace.members]` is the **authoritative** list —
> the tree below is a high-level map and is not kept in lockstep with it (don't hand-sync).
> See `docs/decisions/` for architecture decisions.

```
D:\Project\Sulix Intelligence (Rust workspace — backend)
├── Cargo.toml               ← workspace root (27 members — authoritative list)
├── rust-toolchain.toml      ← pinned Rust toolchain (single source for channel/components/targets)
├── migrations/              ← D1 schema, single source of truth (47 files; numbered 0001…0049)
├── crates/
│   ├── store/               ← D1 access layer + StoreBackend trait + MemoryStore
│   ├── fetcher/             ← RSS/Atom fetch + SSRF guard + AbortSignal timeout
│   ├── rules/               ← Filter/scoring engine (pure logic, unit-tested)
│   ├── ai-pipeline/         ← AI summarization trait + HttpSummarizer
│   ├── intelligence-domain/ ← Pure domain layer (confidence calc + domain ports) — no infra deps
│   ├── search/              ← FTS5 search abstraction + WHERE builder (tested)
│   ├── embedding/           ← Workers AI embedding (bge-large-en-v1.5)
│   ├── vectorize/           ← Shared wasm binding (upsert/query/delete)
│   ├── entity/              ← Entity canonicalizer + classifier (pure logic)
│   ├── api/                 ← HTTP routes (worker::Router)
│   ├── worker-entry/        ← Workers entry (HTTP + Cron + Queue + Metrics)
│   ├── object-store/        ← R2 abstraction (ObjectStore trait + R2Store)
│   ├── event-store/         ← Event sourcing (EventStore trait + D1/R2 backends)
│   ├── intelligence/
│   │   ├── signal-engine/   ← Signal detection + lifecycle (core domain)
│   │   └── reflection-engine/ ← Decision reflection loop
│   ├── memory-engine/       ← Long-term memory promotion
│   ├── context-engine/      ← Context snapshot assembly
│   └── agent-engine/        ← Agent reasoning runtime
```

D:\Project\intel-web (Astro — frontend)
├── astro.config.mjs         ← @astrojs/cloudflare, server mode
├── tailwind.config.mjs      ← "Informed Modernity" design system
├── wrangler.toml             ← Worker config, service binding to API worker
└── src/
    ├── pages/index.astro    ← Latest articles page
    ├── pages/search.astro   ← Search page
    ├── components/          ← Header.astro, ArticleCard.astro
    ├── layouts/Layout.astro ← HTML shell
    ├── infrastructure/api/  ← ApiClient (typed client over service binding env.API)
    └── styles/global.css    ← Tailwind base + fonts
```

## Backend Crate Dependencies（decoupling 现状 2026-09-05 — 详见 decoupling plan + final-architecture-v2）

DDD 目标单向流：`Delivery → Application → Domain ↑ Ports ↑ Infrastructure`。进度：P4 `StoreBackend` body
45→**4**（仅余 GATED decision 写方法，读端 4 方法已删、读 surface 全走 subtrait，supertrait 未删）；P7
架构守卫已入 CI；P5 Phase 1（Source/Entity 编排上收 application）已完成 ——
详见 `docs/superpowers/plans/2026-09-05-decoupling-advance.md`。

```
delivery: worker-entry → api；worker-entry 组装 infrastructure adapters
          → fetcher → worker, feed-rs
          → rules (pure — no worker dep)
          → vectorize (shared wasm binding)
受控引擎（signal/reflection/memory/context/agent/claim-engine）
          → intelligence-domain / shared-kernel / model-runtime + 域内 repository ports
          → 禁止直接依赖 store/vectorize/embedding/event-store/object-store（CI 守卫，见下）
application：services/*.rs（UseCase 编排，generic over 最窄 store subtrait；零 Worker/HTTP/js_sys；
             MemoryStore 单测）—— Source/Entity 已上收
api → store：Source/Entity 经 application::SourceService/EntityService 委托；其余域仍直连 store 且
       handler 自建 `Store::new(...)`（P5b composition-root 注入后 → api → store = 0）
api → search/rules/embedding/vectorize 耦合仍存（P5 收敛目标）
infrastructure adapters（D1XxxRepository / R2 / Vectorize）→ store(D1 access)/embedding/object-store
store → worker (D1Database)
```

## Commands

### Architecture Governance
```bash
cargo deny check bans licenses sources       # 许可证合规 + 依赖重复检查（暂不包含 advisories）
cargo-deny advisories 因 fxhash unmaintained 暂未启用。要启用需先升级或替换 scraper crate。详见 deny.toml。
bash scripts/check-layered-deps.sh           # 分层依赖守卫：受控 crate 禁止新增 store/vectorize/embedding/event-store/object-store 依赖
cargo test -p shared-kernel --test architecture   # P7 跨 crate 边护栏（cargo metadata，无循环）
cargo clippy --workspace -- -D warnings      # 代码质量（遵守 workspace.lints）
cargo fmt --check                            # 格式统一
```

#### 分层依赖守卫（decoupling P1 — 目标已达成 2026-09-05）

`scripts/check-layered-deps.sh` 用 `cargo metadata --no-deps` 读取每个受控 crate 的声明依赖，
断言其不含 banned 基础设施 crate（`store`/`vectorize`/`embedding`/`event-store`/`object-store`）。
**`GRANDFATHERED` 现为空表**（去耦 13→10→8→7→0 收口于 P3-C1/C2 + P6）——受控 crate 不再豁免任何
基础设施依赖，**任何**新增耦合直接 CI 失败。

受控 crate（中间层，不得依赖基础设施）：`signal-engine`、`reflection-engine`、`memory-engine`、
`ai-pipeline`、`context-engine`、`agent-engine`、`claim-engine`。

`cargo-deny` 只能做全局限禁、无法按消费者作用域封禁，故用该脚本补足边缘级约束。去耦总纲、进度与剩余项
（P4 `StoreBackend` body=4 / P5 Phase 1 Source+Entity 上收 / P5b composition-root / Phase 2 域 /
P6 删壳 / GATED decision vertical）见 `docs/superpowers/plans/2026-09-05-decoupling-advance.md`、
`docs/superpowers/plans/2026-08-21-architecture-decoupling-plan.md` 与
`docs/architecture/final-architecture-v2.md`。

#### P7 跨 crate 架构护栏（decoupling — 已入 CI 2026-09-05）

`crates/shared-kernel/tests/architecture.rs` 用 `cargo metadata --no-deps` 断言 DDD 分层的**正常依赖边**
+ 无循环。`GRANDFATHERED` 现 = `application:store` + `api:{store,vectorize,embedding,event-store,
object-store,infrastructure}`（删边即报 removable）。与 `check-layered-deps.sh` 互补：后者封受控引擎
的 banned infra 边（当前空表），前者管 api/application 的暂留边（收紧 = 移除 GRANDFATHERED 条目）。

### Backend (wasm32-unknown-unknown target required)
Toolchain is pinned by `rust-toolchain.toml` (single source — keep CI dtolnay pins in sync).
```bash
cargo check --workspace
cargo test --workspace              # 346 passed（2026-09-05，仅后端；以实际运行为准）
cargo clippy --workspace -- -D warnings
cargo fmt --check
cargo check --workspace --all-features --target wasm32-unknown-unknown   # wasm gate (PR + deploy)
cargo install worker-build@0.8.5    # once per machine (pin exact — keep in sync with [build] + deploy.yml)
cd crates/worker-entry
wrangler dev                        # runs [build] → worker-build --release automatically
wrangler deploy                     # runs [build], then deploys (migrations_dir → ../../migrations)
```
Wrangler's `[build]` in `crates/worker-entry/wrangler.toml` is the **single Worker build entry** — both
`wrangler dev` and `wrangler deploy` run `worker-build --release` there. D1 migrations live in root
`migrations/` (the only copy); do NOT add a `migrations/` dir under `crates/worker-entry/`.

### Frontend
```bash
npm run dev             # astro dev
npm run build           # astro build (to dist/)
npm run test            # vitest (36+ tests)
npm run deploy          # build + wrangler deploy
```

## Key Design Decisions

- **Cloudflare Workers** (not VPS) — solo-operator ops cost, free tier, native D1/Queues/R2
- **D1 with FTS5** (not Postgres/Meilisearch) — only structured data option on CF, external-content FTS5 table via triggers
- **Cloudflare Queues** (not sync cron loop) — per-feed isolation, built-in retry, no time-limit risk
- **Astro server mode + service binding** (not static) — fresh data per-request, no rebuild for new articles
- **worker::Router** (not Axum) — worker::Env/D1Database are not Send/Sync, worker::Router is designed for this
- **SSRF guard** in fetcher blocks IP-literal + localhost-alias URLs; DNS rebinding acknowledged limitation
- **Wrangler `[build]` = single Worker build entry** — `wrangler dev`/`deploy` both run `worker-build --release`; toolchain install is not the hook's job (see ADR-003)
- **Rust toolchain pinned** via `rust-toolchain.toml`; CI dtolnay pins match — bump is an explicit, validated change
- **Root `migrations/` is the only migrations copy** — never add one under `crates/worker-entry/`

## Skills

- `review` — 代码审查
- `qa` — QA 测试
- `ship` — 部署
- `investigate` — 调试问题
