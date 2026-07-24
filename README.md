<p align="center">
  <a href="README.md">🇬🇧 English</a> · <a href="README.zh-CN.md">🇨🇳 中文</a>
</p>

# Sulix Intelligence

> RSS Feed + AI Digest — deployed on Cloudflare Workers.

Fetches RSS/Atom feeds, scores articles with filter rules, summarizes and tags them via DeepSeek V4 Flash, and serves the result as a curated intelligence feed. Pipeline metrics and observability are tracked per-cycle through KV.

## Architecture

```
Cron Trigger (every 30 min) → FETCH_QUEUE → Queue Consumer
  → RSS Fetch → D1 Store → Rules Engine → AI Pipeline → Vectorize Index
  → KV Pipeline Metrics → /api/pipeline/status

HTTP (Worker Router) ←─ service binding ─→ Astro Frontend (Worker)
```

### Crate Dependency Graph

```
worker-entry → api → store → worker (D1, Queues, Router)
            → fetcher → worker, feed-rs
            → rules (pure — no worker dependency)
            → ai-pipeline → store (via StoreBackend trait), Summarizer trait
            → vectorize (shared wasm binding)
api → store, search, rules, embedding, vectorize
store → worker (D1Database)
```

## Crates

| Crate | Purpose |
|---|---|
| `store` | D1 access layer (feeds, articles, CRUD, health) + `StoreBackend` trait + `MemoryStore` for tests |
| `vectorize` | Shared `#[wasm_bindgen]` binding for Cloudflare Vectorize (upsert + query + delete) |
| `fetcher` | RSS/Atom fetch + SSRF guard + full-text extraction (per-feed opt-in) + AbortSignal timeout |
| `rules` | Scoring engine (keyword matches, source filter, AND/OR) — pure logic, unit-tested |
| `ai-pipeline` | `Summarizer` / `HttpClient` traits + `HttpSummarizer` (OpenAI-compatible) + tag normalization |
| `search` | D1 FTS5 keyword search with optional tag/category filters |
| `embedding` | Workers AI embedding provider (bge-large-en-v1.5) |
| `api` | HTTP routes — health, dashboard, tags, feeds CRUD, articles, strategies, pipeline status |
| `worker-entry` | `#[event(fetch/scheduled/queue)]` — Workers entry point + pipeline metrics |

## Key Design Decisions

- **Cloudflare Workers** (not VPS) — solo-operator ops cost, free tier, native D1/Queues/R2
- **D1 with FTS5** (not Postgres/Meilisearch) — only structured data option on CF, external-content FTS5 table via triggers
- **Cloudflare Queues** (not sync cron loop) — per-feed isolation, built-in retry, no time-limit risk
- **Astro server mode + service binding** (not static) — fresh data per-request, no rebuild for new articles
- **worker::Router** (not Axum) — `worker::Env`/`D1Database` are not `Send`/`Sync`; `worker::Router` is designed for this context
- **StoreBackend trait** — `D1Store` (production) and `MemoryStore` (tests) interchangeable via trait; the pipeline is generic over any backend
- **SSRF guard** in fetcher blocks IP-literal + localhost-alias URLs; DNS rebinding is an acknowledged limitation

## Quick Start

```bash
# Backend (wasm32-unknown-unknown target required)
cargo check --workspace
cargo test --workspace              # 100+ unit tests
cargo clippy --workspace -- -D warnings
cargo fmt --check

cargo install worker-build          # one-time
cd crates/worker-entry
worker-build --release
npx wrangler deploy -c wrangler.toml
```

## API

| Endpoint | Description |
|---|---|
| `GET /api/health` | Feed/article/cron stats |
| `GET /api/dashboard` | Health + per-feed stats |
| `GET /api/pipeline/status` | Pipeline health + timing metrics |
| `GET /api/tags` | Aggregated tag cloud with counts |
| `GET/POST /api/feeds` | List / create feed subscriptions |
| `GET/PUT/DELETE /api/feeds/:id` | Read / update / soft-delete |
| `GET /api/articles/latest` | Latest articles (?tag=, ?limit=) |
| `GET /api/articles/trending` | Top-scored (score > 0) |
| `GET /api/articles/search?q=` | FTS5 keyword + semantic search |
| `GET /api/articles/:id` | Article detail |
| `GET /api/articles/:id/content` | Article full-text body (from R2) |
| `GET /api/articles/:id/related` | Related articles by shared tags |
| `GET /api/articles/:id/adjacent` | Previous/next article |
| `GET/POST/PUT/DELETE /api/rules` | Filter/scoring rule CRUD |
| `POST /api/strategies/preview` | Preview strategy impact |
| `POST /api/admin/rebuild-embeddings` | Bulk embedding rebuild |

## CI/CD

Push to `master` → GitHub Actions:
1. `cargo clippy --workspace -D warnings`
2. `cargo test --workspace`
3. `worker-build --release`
4. `wrangler deploy`
5. Smoke tests (health + semantic search)

Secrets: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`

## Frontend

[intel.getsulix.com](https://intel.getsulix.com) — Astro 5 frontend deployed as a Cloudflare Worker with service binding. Features semantic search, dark mode, tag cloud, trending articles, feed management, signal strategies, and bookmarking.

Repo: [weixc0856-cell/Intel-Web](https://github.com/weixc0856-cell/Intel-Web)

## License

MIT
