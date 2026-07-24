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

```
D:\Project\Sulix Intelligence (Rust workspace — backend)
├── Cargo.toml               ← workspace root (9 member crates)
├── migrations/
│   └── 0001_init.sql        ← D1 schema (feeds, articles, filter_rules)
├── crates/
│   ├── store/               ← D1 access layer + StoreBackend trait + MemoryStore
│   ├── fetcher/             ← RSS/Atom fetch + SSRF guard + AbortSignal timeout
│   ├── rules/               ← Filter/scoring engine (pure logic, unit-tested)
│   ├── ai-pipeline/         ← AI summarization trait + HttpSummarizer
│   ├── search/              ← FTS5 search abstraction + WHERE builder (tested)
│   ├── embedding/           ← Workers AI embedding (bge-large-en-v1.5)
│   ├── vectorize/           ← Shared wasm binding (upsert/query/delete)
│   ├── api/                 ← HTTP routes (worker::Router)
│   └── worker-entry/        ← Workers entry (HTTP + Cron + Queue + Metrics)
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
    ├── lib/api.ts           ← Typed API client
    └── styles/global.css    ← Tailwind base + fonts
```

## Backend Crate Dependencies

```
worker-entry → api → store → worker (D1, Queues, Router)
            → fetcher → worker, feed-rs
            → rules (pure — no worker dep)
            → ai-pipeline → store (via StoreBackend trait), Summarizer trait
            → vectorize (shared wasm binding)
api → store, search, rules, embedding, vectorize
store → worker (D1Database)
```

## Commands

### Backend (wasm32-unknown-unknown target required)
```bash
cargo check --workspace
cargo test --workspace              # 90+ unit tests
cargo clippy --workspace -- -D warnings
cargo fmt --check
cargo install worker-build          # need once per machine
worker-build --release              # full Worker build
npx wrangler deploy -c crates/worker-entry/wrangler.toml
```

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

## Skills

- `review` — 代码审查
- `qa` — QA 测试
- `ship` — 部署
- `investigate` — 调试问题
