# Sulix Intelligence

> RSS Feed + AI Digest — deployed on Cloudflare Workers.

Fetches RSS/Atom feeds, scores articles with filter rules, summarizes and tags them via DeepSeek V4 Flash, and serves the result as a curated feed.

## Architecture

```
Cron Trigger → queue → fetch_feed() → D1 (feeds, articles, filter_rules)
  → rules engine (score) → HttpSummarizer (DeepSeek V4) → updated article
  → API (worker::Router) ← service binding ← Astro frontend (Cloudflare Worker)
```

## Crates

| Crate | Purpose |
|---|---|
| `store` | D1 access layer (feeds, articles, CRUD, health) |
| `fetcher` | RSS/Atom fetch + SSRF guard + full-text extraction (per-feed opt-in) |
| `rules` | Scoring engine (keyword matches, source filter, AND/OR) |
| `ai-pipeline` | `Summarizer` trait + `HttpSummarizer` (OpenAI-compatible) |
| `search` | D1 FTS5 keyword search |
| `api` | HTTP routes — health, dashboard, tags, feeds CRUD, articles |
| `worker-entry` | `#[event(fetch/scheduled/queue)]` — Workers entry point |

## Quick Start

```bash
cargo check -p store -p fetcher -p rules -p ai-pipeline -p search -p api
cargo test -p rules

cd crates/worker-entry
worker-build --release
npx wrangler deploy
```

## API

| Endpoint | Description |
|---|---|
| `GET /api/health` | Feed/article/cron stats |
| `GET /api/dashboard` | Health + per-feed stats |
| `GET /api/tags` | Aggregated tag cloud with counts |
| `GET/POST /api/feeds` | List / create feed subscriptions |
| `GET/PUT/DELETE /api/feeds/:id` | Read / update / soft-delete |
| `GET /api/articles/latest` | Latest articles (?tag=, ?limit=) |
| `GET /api/articles/trending` | Top-scored (score > 0) |
| `GET /api/articles/search?q=` | FTS5 keyword search |
| `GET /api/articles/:id` | Article detail |

## CI/CD

Push to `master` → GitHub Actions → `worker-build --release` → `wrangler deploy`

Secrets: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`

## Frontend

[intel.getsulix.com](https://intel.getsulix.com) — Astro 5 frontend deployed as a Cloudflare Worker with service binding.

Repo: [weixc0856-cell/Intel-Web](https://github.com/weixc0856-cell/Intel-Web)
