# Sulix Intelligence — Architecture

## Overview

Sulix is a curated RSS Feed + AI Digest product, deployed entirely on
Cloudflare Workers. It fetches RSS/Atom feeds, scores articles with filter
rules, summarizes and tags them via LLM, and serves the result as a curated
intelligence feed.

## System Diagram

```
Cron Trigger (every 30 min) → FETCH_QUEUE → Queue Consumer
  → RSS Fetch → D1 Store → Rules Engine → AI Pipeline → Vectorize Index
  → KV Pipeline Metrics → /api/pipeline/status

HTTP (Worker Router) ←─ service binding ─→ Astro Frontend (Worker)
```

## Key Design Decisions

- **Cloudflare Workers** (not VPS) — solo-operator ops cost, free tier, native D1/Queues/R2
- **D1 with FTS5** (not Postgres/Meilisearch) — only structured data option on CF
- **Cloudflare Queues** (not sync cron loop) — per-feed isolation, built-in retry
- **Astro server mode + service binding** (not static) — fresh data per-request
- **worker::Router** (not Axum) — `Env`/`D1Database` are not `Send`/`Sync`

## Crate Dependency Graph

```
worker-entry → api → store → worker (D1, Queues, Router)
            → fetcher → worker, feed-rs
            → rules (pure — no worker dependency)
            → ai-pipeline → store (via StoreBackend trait), Summarizer trait
            → vectorize (shared wasm binding)
api → store, search, rules, embedding, vectorize
store → worker (D1Database)
```

## Repository Structure

```
D:\Project\Sulix Intelligence (Rust workspace — 9 crates)
├── Cargo.toml
├── migrations/
│   └── 0001_init.sql
├── crates/
│   ├── store/           D1 access layer + StoreBackend trait
│   ├── fetcher/         RSS/Atom fetch + SSRF guard
│   ├── rules/           Scoring engine (pure logic)
│   ├── ai-pipeline/     LLM summarization + tag normalization
│   ├── search/          FTS5 + semantic search abstraction
│   ├── embedding/       Workers AI (bge-large-en-v1.5)
│   ├── vectorize/       Shared Vectorize wasm binding
│   ├── api/             HTTP routes (worker::Router)
│   └── worker-entry/    Cron + Queue + HTTP entry points

D:\Project\intel-web (Astro frontend — Cloudflare Worker)
├── astro.config.mjs
├── tailwind.config.mjs
└── src/
    ├── pages/           17 route pages + API proxy
    ├── components/      15 Astro components
    ├── layouts/         Base → Marketing / Reader layout
    ├── lib/api/         Domain-split API client
    └── styles/          Tailwind + custom utilities
```

## Pipeline Flow

```
fetcher/          RSS/Atom → full-text extraction → Article
store/            D1 dedup + persist
rules/            Score (keywords, source, AND/OR)
ai-pipeline/      LLM summary + tag normalization
embedding/ + vectorize/  Generate vectors → Vectorize index
KV metrics        Per-cycle timing, article count, LLM calls
```
