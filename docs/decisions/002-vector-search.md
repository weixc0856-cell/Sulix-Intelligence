# ADR-002: Vector Search Architecture

**Date:** 2026-07-24

## Context

Semantic search requires generating embedding vectors for articles and storing
them in a vector database for similarity queries. On Cloudflare Workers, the
available options are Vectorize (native) or using an external provider via HTTP.

## Decision

- **Embedding generation:** Workers AI with `bge-large-en-v1.5` model
  (`crates/embedding/`)
- **Vector storage:** Cloudflare Vectorize via shared wasm binding
  (`crates/vectorize/`)
- **Search:** Dual-path — FTS5 keyword search (`crates/search/`) plus semantic
  search via Vectorize query

The `vectorize` crate exposes the raw `VectorizeIndex` wasm type and a
`meta_value_to_js` helper. All upsert/query/delete operations go through this
crate to avoid duplicating the `#[wasm_bindgen]` extern block.

## Consequences

- Embedding lookup has a cold-start delay on Workers AI (model loading)
- Vectorize queries are limited to the plan's quota
- The `embedding` crate's `embed()` function can only be tested in wasm context
