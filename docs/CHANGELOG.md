# Changelog

## [0.1.0] - 2026-07-24

### Added
- Pipeline observability: timing metrics for fetch/parse/LLM/store/R2 steps, exposed via KV to `/api/pipeline/status`
- StoreBackend trait with MemoryStore test implementation (7 methods, failure injection)
- 28 new pure-function unit tests across api/store/ai-pipeline/search crates
- HTTP fetch timeout via AbortSignal (15s feed / 10s full-text)
- Dashboard Pipeline Health cards (latency bars + embedding coverage)
- PipelineMetrics accumulator with per-cycle snapshot

### Fixed
- 10 error-swallowing sites in worker-entry pipeline: `unwrap_or_default()` replaced with `match`+`console_log`; `let _` replaced with `if let Err(e)`
- `search_count` SQL parameter index bug (missing `idx += 1` in category branch)
- `store/src/lib.rs:59` — `unwrap()` on status filter eliminated
- `api/src/lib.rs:457` — redundant JSON re-parse + `unwrap()` eliminated
- Triplicated VectorizeIndex wasm binding consolidated into shared `crates/vectorize/`
- 6 dead code items removed from vectorize module (VectorEntry, VectorMatch, typed wrappers)

### Changed
- `Store` renamed to `D1Store` with backward-compatible alias
- `process_article` and `process_one_feed` generic over `StoreBackend` trait
- `upsert_vector` from fire-and-forget (`spawn_local`) to awaitable `Result`
- Clippy fixes: type_complexity, adjusted `fmt` throughout

### Removed
- `crates/worker-entry/src/vectorize.rs` — dead code module deleted
- Old V2 architecture entries from CHANGELOG
