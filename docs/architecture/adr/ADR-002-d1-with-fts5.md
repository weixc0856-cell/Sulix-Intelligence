# ADR-002: D1 with FTS5

## Status

Accepted (2026-07)

## Context

The MVP requires structured storage for feeds and articles plus keyword search.
Options evaluated:
- **D1 with FTS5** (Cloudflare's managed SQLite): native to Workers, no external dependency
- **Postgres via Neon/PlanetScale**: adds external dependency, ~$20/mo, not CF-native
- **Meilisearch**: best search quality but adds another service to manage, $30-50/mo
- **Algolia**: same as Meilisearch, plus per-query pricing

## Decision

Use D1 as the primary store with SQLite FTS5 for keyword search.

Schema pattern:
- `articles` table with UNIQUE(feed_id, guid) for dedup via INSERT OR IGNORE
- `articles_fts` as an external-content FTS5 virtual table over (title, ai_summary)
- Triggers on articles (INSERT/UPDATE/DELETE) keep the FTS index in sync
- D1 lacks transactions, so triggers substitute for transactional consistency

## Consequences

Positive:
- Zero additional services — everything lives in D1
- Single SQL query for search: `SELECT ... FROM articles_fts JOIN articles ... WHERE MATCH ?`
- FTS5 ranking (BM25) is good enough for MVP keyword search
- Schema migrations via `wrangler d1 execute`

Negative:
- No Meilisearch-grade typo tolerance or faceted search
- D1 export fails when FTS5 virtual tables exist (must drop/recreate)
- No vector search — requires Vectorize for semantic search (planned)
