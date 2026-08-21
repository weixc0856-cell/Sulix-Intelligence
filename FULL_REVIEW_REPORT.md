# Sulix Intelligence - Full Project Review Report

## Overview

| Dimension | Score | Status |
|-----------|-------|--------|
| **Code Quality (Health)** | 2.5/10 | FAIL |
| **Bug Hunt (QA)** | 6/10 | WARN |
| **Architecture** | See analysis | WARN |
| **Overall Health** | **6.2/10** | -- |

---

## 1. Code Quality (Health Check)

### Compile Status: pass
cargo check succeeded with 0 errors and 0 warnings. All 25 workspace crates compile cleanly.

### Test Results: 289/289 passed
289 `#[test]` across the workspace (baseline 2026-06-22: 255; +34 over 6 weeks). `cargo test --workspace` green across 53 test binaries — 0 failures, 0 panics. A subset (~280) runs on host; the remainder are wasm-targeted binaries that only compile in wasm32 contexts (verified via clippy --all-targets).

### Clippy Analysis: 0 warnings, 0 errors
All 15 warnings cleared (2026-08-21, T1). Fixed: model-runtime (should_implement_trait → impl Default for RoutingPolicy; borrowed_box → &dyn ModelProvider), reasoning-framework (empty_line_after_doc_comments; dead_code seed_count → re-exported as public API `pub use seed::{initial_frameworks, seed_count}`; 3x unnecessary_map_or → is_some_and; collapsible_if; vec_init_then_push → vec![]; items_after_test_module → moved impl before test mod), decision-engine (too_many_arguments → ReconstructDecision hydration struct, 14 args → 1), intelligence-domain (dead_code never-read fields / never-constructed struct → allow(dead_code), Phase 6 removes crate), worker-entry (map_or simplify, needless_borrow). `cargo clippy --workspace --all-targets -- -D warnings` is green.

### Formatting: pass
`cargo fmt --check` green. 6 files were unformatted as of 2026-08-21 (decision-engine/aggregate.rs, events/lib.rs, reasoning-framework/framework.rs, store/domain/article.rs, worker-entry/jobs/backfill.rs, worker-entry/services/http_client.rs) — all fixed.

### Unsafe Blocks: 0

### Unused Dependencies: uuid removed from crates/fetcher/Cargo.toml (2026-08-21, verified unused). wasm-bindgen-futures in crates/vectorize/Cargo.toml is **REQUIRED** — the `#[wasm_bindgen]` attribute on async trait methods (upsert/query/delete in vectorize/src/lib.rs) expands to code referencing `wasm_bindgen_futures`; removing it breaks compilation (E0433). The 2026-06-22 report's claim that it was removable was incorrect.

### Recommendations
- ✅ RESOLVED (2026-08-21, T1): formatting, all clippy warnings, uuid removal.
- Remaining: enabling cargo-deny advisories is still blocked by fxhash unmaintained via scraper crate; upgrading or replacing scraper would unblock this (defer to dependency governance work).
- The `#[allow(dead_code)]` annotations on intelligence-domain (engine.rs claims/signals fields, signal.rs SignalInstance) are temporary — Phase 6 removes the crate entirely per `docs/architecture/final-architecture-v2.md`.

---

## 2. Bug Hunt & Security (QA)

**Score: 6/10**

**Summary: 10 issues (P0: 0, P1: 1, P2: 5, P3: 4)**


---

## P1 - High (confidence: 9/10) crates\worker-entry\src\services\http_client.rs:62
**Production panic via expect() in setTimeout lookup**
`js_sys::Reflect::get(&global, &"setTimeout".into()).expect("setTimeout global")` will panic with an uncaught error if the JavaScript `setTimeout` global is absent or inaccessible. While Cloudflare Workers expose setTimeout via web-sys bindings, any environment mismatch (sandbox change, API deprecation, test harness) causes an immediate Worker crash with no recovery path.

Fix: Replace expect() with a Result/ Option chain that falls back to a no-op or returns an error. Example: `js_sys::Reflect::get(&global, &"setTimeout".into()).ok().and_then(|v| v.dyn_into::<js_sys::Function>().ok()).ok_or_else(|| HttpClientError::Network("setTimeout not available".into()))?`

---

### P2 - Medium (confidence: 10/10) crates\store\src\domain\article.rs:204
**LLM-sourced tag values embedded in SQL via format! instead of bind parameters**
`related_articles` builds SQL LIKE clauses by formatting escaped tag values directly into the SQL string (`format!("ai_tags LIKE ...", ...)`) rather than using prepared-statement bind parameters. While the `escape_like` + `replace('\'', "''")` escaping currently prevents injection, this breaks defense-in-depth — a single escaping oversight opens SQL injection. Tags originate from LLM output (untrusted by design, per the "LLM output trust boundary" review directive).

Fix: Move the LIKE pattern entirely into a bind parameter. Remove the `format!` + `replace` chain and use `?N` placeholders for each LIKE pattern, same as the rest of the codebase does (e.g., `escape_like` + bind in `articles_by_tag`).

---

### P2 - Medium (confidence: 8/10) crates\decision-engine\src\aggregate.rs:176
**Unconditional unwrap() on observed_outcomes.last() in production code**
`attach_outcome` pushes to `self.observed_outcomes` then immediately calls `.last().unwrap()` on it. While always safe under the current ordering, this is a fragile panic point — any future refactor that moves the push or rearranges lines will cause a production panic. The method `complete()` correctly checks `is_empty()` first; `attach_outcome` does not.

Fix: Replace `.last().unwrap()` with `.last().expect(...)` with a descriptive message, or capture the just-pushed value in a local variable before pushing: `let m = cmd.outcome.metric.clone(); self.observed_outcomes.push(cmd.outcome);` then use `m` directly.

---

### P2 - Medium (confidence: 9/10) crates\store\src\domain\feed.rs:80
**Dynamic UPDATE SQL built with format! across three CRUD modules**
`update_feed` (feed.rs:80), `update_rule` (feed_rules.rs:83), and `update_reflection` (reflection/crud.rs:76) all construct UPDATE statements via `format!("UPDATE ... SET {}", parts.join(", "))`. While column segments are hardcoded in match arms, this pattern circumvents compile-time SQL verification and is a maintenance hazard — any future change that introduces a user-controlled column name would create a SQL injection vulnerability.

Fix: Keep the dynamic-column pattern but isolate it behind a helper that validates column names against an allowlist (enum or const array). Alternatively, build separate prepared statements for each column combination.

---

### P2 - Medium (confidence: 8/10) crates\worker-entry\src\services\http_client.rs:118
**unreachable!() macro in HTTP retry loop will panic if control flow changes**
The `execute_with_retry` method ends its retry loop with `unreachable!()`. While every path inside the loop currently returns, this is fragile — adding a `break` or removing a `continue` anywhere in the loop body will trigger a production panic. The Rust compiler does not enforce that all `for` loop paths return.

Fix: Replace the `for` loop with a `loop { ... }` that uses `break` to exit, or replace `unreachable!()` with `Err(HttpClientError::Network("retry loop exhausted without returning".into()))` as a defensive fallback.

---

### P2 - Medium (confidence: 7/10) crates\worker-entry\src\runtime\signal.rs:26
**Silent error swallowing on critical infrastructure bindings via .ok()**
Multiple worker-entry modules use `.ok()` to silently discard binding errors: `env.bucket("RAW_CONTENT").ok()`, `env.get_binding::<VectorizeIndex>("VECTORIZE").ok()`, `env.kv("CACHE").ok()`. When a binding is misconfigured, the pipeline silently degrades — full-text extraction, vector search, and checkpoint persistence fail without logging the root cause. The binding failure is invisible in production logs.

Fix: Log a warning when a binding fails instead of using `.ok()`. For example: `env.bucket("RAW_CONTENT").map_err(|e| console_log!("RAW_CONTENT bucket not bound: {e}")).ok()`

---

#### P3 - Low (confidence: 8/10) crates\worker-entry\src\runtime\cron.rs:16
**Cron feature flags silently default to disabled on env var parse failure**
The `CronConfig::from_env` closure uses `env.var(key).ok().and_then(|v| v.to_string().parse().ok()).unwrap_or(false)`. If the environment variable is set to an invalid value (e.g., "ture" instead of "true"), parsing fails silently and the feature appears disabled with no log message, making debugging difficult.

Fix: Add a log warning when parsing fails: after `unwrap_or(false)`, check if the var was set but unparseable. Or use a `match` on the parse result to log malformed values.

---

#### P3 - Low (confidence: 9/10) crates\store\src\domain\feed_analytics.rs:61
**unwrap_or(0) and unwrap_or_default() mask database query failures**
Multiple methods in feed_analytics.rs use `unwrap_or(0)` and `unwrap_or_default()` on `first::<Value>()` results (lines 61, 66, 68, 75, 82, 88). When a query returns `None`, these silently substitute zero rather than propagating the error, making transient DB failures invisible in logs.

Fix: Consider logging or returning a `Result` instead of silently defaulting. If a `None` result is a legitimate edge case (empty table), document it explicitly.

---

#### P3 - Low (confidence: 8/10) crates\intelligence\reflection-engine\src\service.rs:173
**Critical operation results suppressed with `let _ =` prefix**
Reflection engine service suppresses `update_reflection`, `insert_outbox`, and `event_store.append_event` results using `let _ =`. These are dual-write persistence operations where a silent failure could lead to inconsistency between D1 state and the event archive. The `_ =` pattern intentionally discards both success and failure information.

Fix: At minimum log errors on each operation. For the D1 `update_reflection` call that persists the final reflection result (line 173), treat failure as a hard error rather than silently continuing.

---

#### P3 - Low (confidence: 7/10) crates\search\src\lib.rs:52
**FTS5 query accepts arbitrary user input with no input-size or syntax guard**
The FTS5 MATCH query at line 52 passes user-supplied queries directly as bind parameters. While SQL injection is not possible (parameterized), FTS5's own syntax (`*`, `"phrase"`, `term1 NEAR term2`, boolean operators, prefix queries) can be abused: `*` matches every row, and complex boolean queries could degrade D1 performance. There is no query-length limit or syntax validation.

Fix: Add a query-length cap (e.g., 200 chars), strip or escape FTS5 special characters for simple searches, or validate the query against a safe pattern before passing to MATCH.

---

## 3. Architecture Review

### Overview
The Sulix Intelligence Rust backend comprises 29 workspace crates across ~308 source files. The architecture follows trait-based DDD with a StoreBackend supertrait (being decomposed) as the central persistence boundary. Worker-entry is the composition root; api provides HTTP routes; modular engine crates (signal, reflection, memory, context, agent) implement intelligence features. The codebase is well-structured with extensive test infrastructure (MemoryStore, NoopProvider, BlobStore). However, several crates (intelligence-domain, reasoning-framework, events, claim-engine) are declared but unwired, and the entity crate appears unused. The StoreBackend trait (50+ methods) is acknowledged technical debt targeted for Sprint 6.2 removal. There are 15 #[allow(dead_code)] annotations across the codebase. The dependency graph has no circular dependencies and a max depth of 4 within the workspace. Pure-logic crates (rules, entity, shared-kernel) have zero Worker dependencies, enabling standard unit testing.

### Module Dependency Graph
- Total modules: 29
- Dependency chain depth: 4
- Circular dependencies: None (clean)
- Orphan modules: intelligence-domain, reasoning-framework, events, claim-engine

### Dead Code

- [LOW] truncate_chars function (d:\Project\Sulix Intelligence\crates\worker-entry\src\utils.rs): Annotated #[allow(dead_code)] — truncate_body is used instead. The function was likely kept for future use but is currently dead code.
- [LOW] RelationEdge struct in MemoryStore (d:\Project\Sulix Intelligence\crates\store\src\memory\mod.rs): Annotated #[allow(dead_code)] on line 116. This struct is defined but never constructed or read; entity_relation_edges are inserted as raw tuples.
- [LOW] BriefingArtifactEnvelope fields (d:\Project\Sulix Intelligence\crates\api\src\briefing.rs): Fields schema_version and artifact_type are annotated #[allow(dead_code)]. They exist only for JSON deserialization structure but are never read after parsing.
- [MEDIUM] intelligence-domain crate (entire crate) (d:\Project\Sulix Intelligence\crates\intelligence-domain\src\lib.rs): No crate in the workspace depends on intelligence-domain. Despite claiming 'the old claim-engine and signal-engine crates now re-export from here', neither actually does. This is aspirational code that compiles but is never called.
- [MEDIUM] reasoning-framework crate (entire crate) (d:\Project\Sulix Intelligence\crates\reasoning-framework\src\lib.rs): No crate depends on reasoning-framework. The framework models (CalibrationEngine, ReasoningSelector, ReasoningFramework, etc.) compile but are never instantiated or called from any production code path.
- [MEDIUM] events crate (entire crate) (d:\Project\Sulix Intelligence\crates\events\src\lib.rs): The Events crate (EventEnvelope wrapper for IntelligenceEvent queue messages) has no consumers. No crate lists it as a dependency despite it being declared in the workspace.
- [MEDIUM] claim-engine crate (entire crate) (d:\Project\Sulix Intelligence\crates\claim-engine\src\lib.rs): Marked DEPRECATED (Sprint 6.2D) but no crate depends on it, so its deprecation is already complete. The crate compiles but all public types (LlmClaimExtractor, ClaimExtractor, evaluate_claim_confidence) are unused.
- [LOW] SemanticDiscoverySource in signal-engine (d:\Project\Sulix Intelligence\crates\intelligence\signal-engine\src\discovery\mod.rs): The semantic discovery pipeline (clustering, converter, retrieval, similarity modules) is declared but the SemanticDiscoverySource is only referenced in worker-entry/src/jobs/signal.rs behind an option check. If VECTORIZE binding is absent, it's never used.
- [LOW] signal-engine converter module items (d:\Project\Sulix Intelligence\crates\intelligence\signal-engine\src\discovery\converter.rs): Contains #[allow(dead_code)] annotation. The converter transforms clusters to signal candidates but is only used when semantic discovery is active.
- [LOW] signal-engine retrieval module items (d:\Project\Sulix Intelligence\crates\intelligence\signal-engine\src\discovery\retrieval.rs): Contains #[allow(dead_code)] annotation. Article retrieval for semantic discovery is behind optional Vectorize dependency.

### Technical Debt

- [HIGH] (effort: LARGE) StoreBackend supertrait with 50+ methods -> Complete the Sprint 6.2 migration: decompose StoreBackend into separate repository + query service traits per bounded context. Remove StoreBackend entirely. The d1_delegate.rs and backend.rs already show the direction, but the gordian knot of all 20 subtraits + StoreBackend legacy methods must be severed.
- [HIGH] (effort: MEDIUM) Duplicate event envelope types (4 variants) -> Consolidate to the canonical IntelligenceEvent + a single transport envelope. Currently shared-kernel defines IntegrationEvent + IntelligenceEvent + DecisionDomainEvent + SignalDomainEvent, events crate defines another EventEnvelope, event-store defines a third EventEnvelope. Unify into one envelope used everywhere.
- [MEDIUM] (effort: LOW) No lib.rs in key crates (binary-only crate limitations) -> Several crates (fetcher, rules, search, embedding, entity) have only src/lib.rs properly. But the pattern is used inconsistently — some crates like signal-engine use src/lib.rs only while having deep module trees. Recommend ensuring every non-entry crate has proper lib.rs with doc-level architecture comments.
- [MEDIUM] (effort: LARGE) MemoryStore maintenance burden (20 RefCell fields, 50+ method impls) -> As StoreBackend is decomposed, MemoryStore should also be split into per-domain test stores. Each test store would implement only the relevant repository/query trait for its domain, reducing the monolith.
- [MEDIUM] (effort: MEDIUM) Duplicate HttpClient trait definitions -> ai-pipeline defines its own HttpClient trait; model-runtime defines another HttpClient (through RealDeepSeek). WorkerHttpClient in worker-entry implements the former. Unify these into a single HttpClient trait in a shared location (perhaps in shared-kernel or model-runtime).
- [MEDIUM] (effort: MEDIUM) Scattered LLM prompt construction -> Prompts are built in ai-pipeline/lib.rs (build_summarize_prompt), ai-pipeline/briefing/prompt.rs (SYSTEM_PROMPT constant), reflection-engine/generator/prompt.rs. Recommendation: centralize prompt templates in a single location (perhaps model-runtime) with a PromptBuilder trait.
- [MEDIUM] (effort: MEDIUM) N+1 query problem in signal detail -> load_signal_detail in store/domain/signal/detail.rs fires sequential queries for thread, instances, evidence, entities, and related signals. The SignalQueryService and application/radar.rs were created to address this with batched queries, but not all consumers are migrated.
- [LOW] (effort: MEDIUM) Mixed error handling style (thiserror enums vs String) -> Low-level crates (store, fetcher, vectorize) use proper thiserror enums. Higher-level orchestration (reflection-engine, signal-engine pipeline) uses Result<_, String> or Result<_, format!()>. Migrate to typed errors throughout.
- [LOW] (effort: MEDIUM) Config flows through raw env.var() calls everywhere -> Configuration is extracted via env.var('KEY') and env.secret('KEY') scattered across handlers and services. Only IntelligenceRuntime enforces a struct-based config. Create a centralized AppConfig struct loaded at startup.
- [LOW] (effort: MEDIUM) Non-idiomatic SQL string building with format!() -> Many D1 queries use format!() for dynamic SQL (e.g., query_lineage in provenance.rs, feeds_due_for_fetch in feed.rs). Risk of SQL injection if user-supplied strings reach these. Prefer parameterized queries throughout.

### Design Observations

- + Extensive trait-based abstraction for testability: StoreBackend, EventStore, ObjectStore, ModelProvider, Summarizer, EmbeddingProvider, HttpClient, ArtifactRegistry, SignalSource, and ReflectionGenerator are all trait-defined, enabling MemoryStore/NoopProvider/BlobStore test doubles.
- + Pure logic crates with zero Worker dependencies: rules, entity (canonicalizer, classifier, models), shared-kernel, model-runtime, content-governance, decision-engine, intelligence-domain, reasoning-framework, and events crates have no dependency on worker or wasm-bindgen, enabling standard cargo test without wasm target.
- + Clear DDD layering with domain event provenance: The codebase has a well-defined layering: repository traits in store/traits/repo, query services in store/traits/query, event-sourced aggregates in decision-engine, event store in event-store, and an outbox-first consistency model.
- + MemoryStore provides failure injection capabilities: MemoryStore has boolean flags (fail_insert, fail_rules, fail_summary, fail_fetch_result, fail_r2_key) that let tests exercise error-handling paths without creating a real D1 database.
- + Deprecation headers with planned migration timelines: Both claim-engine and signal-engine have ASCII banner comments explaining they are DEPRECATED in Sprint 6.2D with migration paths to intelligence-domain. StoreBackend has a similar deprecation notice.
- + Signal engine uses incremental checkpoint via KV: worker-entry/src/jobs/signal.rs implements a KV-backed checkpoint ('signal_engine:last_run') that skips the engine cycle when no new articles have been ingested since the last run. This is the primary D1 write-amplification guard.
- + No circular dependencies in the entire workspace: Despite 29 interdependent crates, there are zero circular dependency chains. The dependency graph is a strict DAG with shared-kernel at the bottom and worker-entry at the top.
- + Anti-corruption layer pattern in d1_delegate.rs: The d1_delegate.rs file implements every domain trait for D1Store, acting as an anti-corruption layer. Each trait method delegates 1:1 to an existing D1Store method, enabling incremental migration.
- - Multiple event envelope types cause conceptual confusion: There are at least 4 distinct event envelope types: shared-kernel's IntegrationEvent, event-store's EventEnvelope, events crate's EventEnvelope, and decision-engine's DecisionDomainEvent. A developer must understand which envelope to use in which context.
- - Deep module inheritance in store/domain: The store crate has grown to contain not just the D1 access layer but also domain logic for signals, decisions, entities, observations, claims, reflections, briefings, artifacts, memory, and context snapshots. This violates the store's stated purpose as 'the D1 access layer.'

### Prioritized Recommendations

- **[P1]** Complete StoreBackend decomposition into per-domain traits: The 50-method StoreBackend supertrait composes ~20 smaller traits but is still the primary interface used across the codebase. Its deprecation (Sprint 6.2) creates a migration bottleneck. Completing the decomposition will enable smaller, more testable units and eventually allow removing d1_delegate.rs (currently 774 lines of forwarding methods).
- **[P1]** Remove or wire orphan crates (intelligence-domain, reasoning-framework, events, claim-engine): Four workspace members compile but have zero consumers. This adds ~13 minutes to CI compilation for no benefit. Either wire them into the production pipeline or remove them from workspace membership.
- **[P1]** Consolidate event envelope types to a single canonical format: Having 4 different event envelope types (shared-kernel IntegrationEvent, event-store EventEnvelope, events crate EventEnvelope, decision-engine DecisionDomainEvent) creates confusion about which to use. Unify around shared-kernel's IntelligenceEvent with a single transport envelope.
- **[P2]** Unify HttpClient implementations: The ai-pipeline crate defines its own HttpClient trait for embeddings, while model-runtime defines another for LLM calls. WorkerHttpClient in worker-entry implements only the ai-pipeline version. A single HttpClient trait in a shared location would eliminate duplication and ensure consistent retry/timeout logic.
- **[P2]** Centralize LLM prompt construction: Prompt templates are scattered across ai-pipeline (summarize), ai-pipeline/briefing (briefing), and reflection-engine (reflection). A PromptBuilder trait or centralized prompt catalog would prevent drift and enable prompt versioning/experimentation.
- **[P2]** Address N+1 query patterns in signal detail loading: load_signal_detail fires 5+ sequential queries. The SignalQueryService and application/radar.rs batched approach exists but isn't universally adopted. Profile and migrate remaining callers.
- **[P2]** Remove dead code with #[allow(dead_code)] annotations: 15 #[allow(dead_code)] annotations exist across the codebase. Most are structural fields for deserialization or functions superseded by alternatives. Clean them up to improve code clarity and compiler warning effectiveness.
- **[P3]** Introduce a centralized AppConfig struct for environment configuration: Currently, env.var()/env.secret() calls are scattered across handlers and services. A single AppConfig struct loaded at startup would make dependencies explicit, document required env vars, and simplify testing.
- **[P3]** Standardize error types in higher-level orchestration code: Low-level crates use proper thiserror enums, but reflection-engine, signal-engine pipeline, and some API handlers use Result<_, String>. Adopt thiserror consistently throughout.
- **[P3]** Use parameterized SQL queries instead of format!()-based SQL strings: Several D1 queries in provenance.rs and feed.rs build SQL with format!(). While the interpolated values are not user-supplied today, this pattern is fragile and could become a SQL injection vector. Use ?N bind parameters consistently.

---

## 4. Consolidated Action Items

### Immediate (P1)

- [QA] crates\worker-entry\src\services\http_client.rs:62 - Production panic via expect() in setTimeout lookup
- [ARCH] Complete StoreBackend decomposition into per-domain traits
- [ARCH] Remove or wire orphan crates (intelligence-domain, reasoning-framework, events, claim-engine)
- [ARCH] Consolidate event envelope types to a single canonical format
- [HEALTH] Fix 16 unformatted files: run `cargo fmt` to auto-correct formatting across api, claim-engine, decision-engine, infrastructure, intelligence-domain, model-runtime, reasoning-framework, and store crates.
- [HEALTH] Address clippy warnings: (1) reasoning-framework/framework.rs has 4 collapsible/simplification warnings — replace map_or(false, pred) with is_some_and(pred) and collapse nested if; (2) reasoning-framework/seed.rs: use vec![] macro instead of push-after-create; (3) reasoning-framework/calibration.rs: remove empty line after doc comment; (4) model-runtime/gateway.rs: implement Default trait or rename default(); replace &Box<dyn ModelProvider> with &dyn ModelProvider; (5) decision-engine/aggregate.rs: reduce reconstruct() parameter count from 14 via builder pattern or struct; (6) intelligence-domain: remove or use dead fields/structs (claims, signals, SignalInstance); (7) reasoning-framework: remove or use seed_count() if unused.
- [HEALTH] Remove unused dependencies: uuid from fetcher (was likely planned for article ID generation but never wired); wasm-bindgen-futures from vectorize (not needed there — worker-entry has its own copy).

### Follow-up (P2-P3)

- [QA] LLM-sourced tag values embedded in SQL via format! instead of bind parameters (crates\store\src\domain\article.rs:204)
- [QA] Unconditional unwrap() on observed_outcomes.last() in production code (crates\decision-engine\src\aggregate.rs:176)
- [QA] Dynamic UPDATE SQL built with format! across three CRUD modules (crates\store\src\domain\feed.rs:80)
- [QA] unreachable!() macro in HTTP retry loop will panic if control flow changes (crates\worker-entry\src\services\http_client.rs:118)
- [QA] Silent error swallowing on critical infrastructure bindings via .ok() (crates\worker-entry\src\runtime\signal.rs:26)
- [ARCH] Unify HttpClient implementations
- [ARCH] Centralize LLM prompt construction
- [ARCH] Address N+1 query patterns in signal detail loading
- [ARCH] Remove dead code with #[allow(dead_code)] annotations
- [HEALTH] Consider enabling cargo-deny advisories: currently blocked by fxhash unmaintained via scraper crate; upgrading or replacing scraper would unblock this.
- [HEALTH] The dead_code warnings indicate potential unfinished or dead code paths — review reasoning-framework/src/seed.rs seed_count(), intelligence-domain/src/engine.rs (claims/signals fields), and intelligence-domain/src/signal.rs SignalInstance struct to decide if they should be removed or completed.

---

## 5. Baseline for Trend Tracking

| Metric | 2026-06-22 | 2026-08-21 (T1) |
|--------|------------|------------------|
| Health Score | 2.5/10 | 2.5/10 (defer re-scoring to post-decoupling) |
| QA Score | 6/10 | 6/10 |
| Tests Passing | 255/255 | 289 `#[test]`, all green (53 binaries, 0 failures) |
| Clippy Warnings | 12 | 0 (`-D warnings` green) |
| Formatting | 16 files | pass |
| Dead Code Items | 10 | 10 (unchanged count; intelligence-domain flagged for Phase 6 removal) |
| Tech Debt Items | 10 | 10 |
| Architecture | dual-track | FROZEN v2 plan — see `docs/architecture/final-architecture-v2.md` |

---
*Original generated by gstack /review with ultracode — 2026-06-22. Baseline updated 2026-08-21 by Sprint 6.5 T1.*
