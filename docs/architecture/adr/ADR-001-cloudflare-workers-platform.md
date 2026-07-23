# ADR-001: Cloudflare Workers Platform

## Status

Accepted (2026-07)

## Context

Sulix Feed needs to be deployable by a solo operator with minimal ops overhead.
The previous V1 architecture ran as a native Rust CLI — it required a server or
CI runner, manual scheduling, and local SQLite storage that didn't scale to a
multi-user product.

A VPS-based deployment (e.g. DigitalOcean, Hetzner) was considered but rejected
because:
- A solo operator cannot afford pager-duty rotation
- PostgreSQL/VPS adds ~$15-30/mo before any revenue
- Free tiers of serverless platforms cover the expected MVP traffic

## Decision

Deploy on Cloudflare Workers (serverless edge runtime).

The entire stack lives within Cloudflare's ecosystem:
- **Workers**: Rust via `workers-rs` (wasm32-unknown-unknown) for the API; Astro via `@astrojs/cloudflare` for the frontend
- **D1**: Managed SQLite for structured data (feeds, articles, filter rules)
- **Queues**: Async fan-out for feed fetching with per-message retry/DLQ
- **R2**: Object storage for raw article content (planned)
- **Vectorize**: Vector embeddings for semantic search (planned)

## Consequences

Positive:
- Zero infrastructure management — the platform handles scaling, TLS, DDoS
- Generous free tier (100k requests/day, 1M reads/day on D1)
- Service bindings let the frontend call the API Worker without public HTTP
- Built-in cron triggers, queue consumer bindings

Negative:
- `worker::Env` and `D1Database` are `!Send`/`!Sync` — must use `worker::Router` instead of Axum's `Handler` trait
- Wasm target has no native TCP sockets — `reqwest` doesn't work, must use `worker::Fetch`
- D1 has no transaction support — FTS5 index sync requires triggers instead
- Cannot easily migrate to another cloud provider (vendor lock-in)
