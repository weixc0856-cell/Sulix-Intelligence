# ADR-001: StoreBackend Trait

**Date:** 2026-07-24

## Context

The pipeline needs to operate against both a real D1 database (production) and
an in-memory store (tests). The Workers D1 API is only available in wasm
contexts, making tests that touch storage impossible to run on native targets.

## Decision

Introduce a `StoreBackend` trait with two implementations:

- `D1Store` — production, wraps `worker::D1Database`
- `MemoryStore` — tests, uses in-memory `HashMap`s with failure injection

The pipeline functions `process_article` and `process_one_feed` are generic
over `StoreBackend`, enabling unit tests without a Workers runtime.

## Consequences

- `+62` lines of trait definition (`crates/store/src/backend.rs`)
- `+163` lines of MemoryStore (`crates/store/src/memory.rs`)
- All pipeline orchestration can now be tested in `cargo test`
