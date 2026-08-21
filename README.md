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

The codebase is being migrated to **Ports & Adapters** (strict one-way dependencies):

```
delivery (api + worker-entry)
      ↓ 只依赖 application + 领域类型 + worker
application（用例服务，泛型注入端口）
      ↓
domain（intelligence-domain / decision-engine / reasoning-framework / shared-kernel）
      ↑ 端口（Repository traits 定义在领域层）
infrastructure（D1 适配器，实现领域 trait）──→ store（仅作为 D1 数据访问库）
```

`store` is being dismantled from a 5800-line god-object into a pure D1 data-access layer. Migration status and the P2–P7 / T6–T9 roadmap are tracked in `docs/superpowers/plans/` (see [Migration Status](#migration-status)).

## Cognitive Pipeline

```
Source Registry → Content Policy → Observation
         ↓
     Signal Detection
         ↓
     Claim Extraction
         ↓
     Decision Engine → Outcome → Reflection
         ↓
     Confidence Engine (factor-based, interpretable)
         ↓
     Memory Engine → Context Engine
```

## Crates

| Crate | Purpose |
|---|---|
| `api` | 73+ HTTP routes (delivery layer) — health, dashboard, signals, decisions, claims, confidence, sources, observations, compliance, graph, feeds CRUD, articles, strategies |
| `worker-entry` | `#[event(fetch/scheduled/queue)]` — Workers entry point + pipeline metrics |
| `application` | Use-case services (port-injected) — radar projection, decision graph, semantic search |
| `intelligence-domain` | Intelligence domain types + `Observation/Claim/Signal` repository ports |
| `decision-engine` | Decision aggregate, status machine + `DecisionRepository` port |
| `reasoning-framework` | Framework/applied-trace types for reasoning explanations |
| `claim-engine` | Claim extraction from signals |
| `shared-kernel` | Shared value objects/IDs/events across intelligence crates |
| `infrastructure` | D1 adapters implementing domain repository traits (`decision/signal/claim/reflection/memory_repository.rs`) |
| `store` | D1 data-access layer (former god-object; being shrunk, `StoreBackend` `#[deprecated]` with a hard TTL) |
| `intelligence/signal-engine` | Signal discovery, clustering, scoring, radar projection, batch evidence/entity queries |
| `intelligence/reflection-engine` | Decision reflection lifecycle — generates structured lessons from outcomes |
| `memory-engine` | Long-term memory promotion — evaluates, scores, and archives memory artifacts |
| `context-engine` | Intent-aware context snapshot assembly for agent queries |
| `agent-engine` | Agent reasoning runtime with evidence validation |
| `model-runtime` | LLM abstraction (`Summarizer`/`HttpClient` traits + `MockProvider` for tests) |
| `content-governance` | Pure-logic policy evaluation — storage/serving/embedding/AI permissions per source tier |
| `fetcher` | RSS/Atom fetch + SSRF guard + full-text extraction (per-feed opt-in) + AbortSignal timeout |
| `rules` | Scoring engine (keyword matches, source filter, AND/OR) — pure logic, unit-tested |
| `search` | D1 FTS5 keyword search with optional tag/category filters |
| `entity` | Entity value object + repository |
| `embedding` | Workers AI embedding provider (bge-large-en-v1.5) |
| `vectorize` | Shared `#[wasm_bindgen]` binding for Cloudflare Vectorize (upsert + query + delete) |
| `events` | Event contracts (outbox event definitions) |
| `event-store` | Event sourcing append/read — outbox-first writes to event store, async drain to R2 |
| `object-store` | `ObjectStore` trait + `R2Store` for artifact blob storage |
| `ai-pipeline` | LLM summarization + tag normalization |

## Key Design Decisions

- **Cloudflare Workers** (not VPS) — solo-operator ops cost, free tier, native D1/Queues/R2
- **D1 with FTS5** (not Postgres/Meilisearch) — only structured data option on CF, external-content FTS5 table via triggers
- **Cloudflare Queues** (not sync cron loop) — per-feed isolation, built-in retry, no time-limit risk
- **Astro server mode + service binding** (not static) — fresh data per-request, no rebuild for new articles
- **CQRS with Event Sourcing** — separate write (Repository traits) and read (QueryService traits); events stored append-only with outbox pattern
- **Factor-based Confidence** — confidence = geometric mean of evidence × source_trust × freshness × calibration; fully interpretable, not a black-box LLM score
- **Source Governance** — every feed has a source registry entry with tier, policy, license, trust_score; content policy enforced at ingestion + serving gates
- **Provenance Chain** — every intelligence artifact carries Source → Observation → Signal → Claim → Decision → Memory lineage
- **worker::Router** (not Axum) — `worker::Env`/`D1Database` are not `Send`/`Sync`; `worker::Router` is designed for this context
- **StoreBackend trait** — `D1Store` (production) and `MemoryStore` (tests) interchangeable via trait; the pipeline is generic over any backend (being dismantled into per-domain ports)

## Quick Start

```bash
# Backend (wasm32-unknown-unknown target required)
cargo check --workspace
cargo test --workspace              # 318+ unit tests
cargo clippy --workspace -- -D warnings
cargo fmt --check

cargo install worker-build          # one-time
cd crates/worker-entry
worker-build --release
npx wrangler deploy -c wrangler.toml
npx wrangler d1 migrations apply sulix-feed-db --remote  # apply schema migrations
```

## API

### System
| Endpoint | Description |
|---|---|
| `GET /api/health` | Feed/article/cron stats |
| `GET /api/dashboard` | Health + per-feed stats |
| `GET /api/pipeline/status` | Pipeline health + timing metrics |
| `GET /api/stats` | Score distribution + article trend |
| `GET /api/tags` | Aggregated tag cloud with counts |
| `GET /api/categories` | Category listing with article counts |
| `GET /api/intelligence/trust` | Trust Center — accuracy, source reliability, evaluation summary |

### Articles & Feeds
| Endpoint | Description |
|---|---|
| `GET /api/articles/latest` | Latest articles (?tag=, ?category=, ?limit=) |
| `GET /api/articles/trending` | Top-scored (score > 0) |
| `GET /api/articles/search?q=` | FTS5 keyword + semantic search |
| `GET /api/articles/batch?ids=` | Batch fetch by IDs |
| `GET /api/articles/:id` | Article detail with provenance |
| `GET /api/articles/:id/content` | Article full-text (from R2, policy-gated) |
| `GET /api/articles/:id/related` | Related articles by shared tags |
| `GET /api/articles/:id/adjacent` | Previous/next article |
| `GET/POST/PUT/DELETE /api/feeds` | Feed subscription CRUD |

### Intelligence
| Endpoint | Description |
|---|---|
| `GET /api/intelligence/signals` | Today's signals summary |
| `GET /api/intelligence/radar` | Radar dashboard with health/trend/evidence |
| `GET /api/intelligence/signals/:id` | Signal detail (timeline, evidence, entities) |
| `GET /api/intelligence/signals/:id/provenance` | Signal provenance chain |
| `GET /api/intelligence/threads/:id` | Signal thread detail |
| `GET /api/intelligence/briefing/today` | Today's AI-generated intelligence brief |
| `GET /api/intelligence/briefings` | Briefing history |
| `GET /api/intelligence/entities` | Entity graph listing |
| `GET /api/intelligence/entities/:id/*` | Entity detail, articles, signals, relations, activity |

### Decision Intelligence
| Endpoint | Description |
|---|---|
| `GET /api/intelligence/decisions` | Decision list (?status=) |
| `POST /api/intelligence/signals/:id/decisions` | Create decision for signal |
| `GET /api/intelligence/decisions/stats` | Decision accuracy dashboard |
| `GET /api/intelligence/decisions/:id` | Decision detail |
| `POST /api/intelligence/decisions/:id/status` | Update decision status |
| `POST /api/intelligence/decisions/:id/reflect` | Trigger AI reflection |
| `POST /api/intelligence/decisions/:id/outcomes` | Record outcome |
| `POST /api/intelligence/decisions/:id/evaluations` | Record evaluation |
| `GET /api/intelligence/decisions/:id/timeline` | Merged decision timeline |
| `GET /api/intelligence/decisions/:id/explanation` | Explain why the system believes this |
| `GET /api/projections/decision-graph` | Cognitive graph projection |
| `POST /api/projections/decision-graph/:id/expand` | Expand graph node |

### Claims & Confidence
| Endpoint | Description |
|---|---|
| `GET /api/claims/:id` | Claim detail with evidence |
| `GET /api/confidence/:entity_type/:entity_id` | Confidence history timeline |

### Governance
| Endpoint | Description |
|---|---|
| `GET/POST/PUT/DELETE /api/sources` | Source registry CRUD |
| `GET /api/observations` | Observation list |
| `GET /api/observations/:id/lineage` | Full provenance lineage |
| `POST /api/compliance/takedown` | Submit takedown request |
| `GET /api/compliance/takedowns` | List takedown requests (admin) |

### Internal
| Endpoint | Description |
|---|---|
| `POST /api/internal/agent/run` | Agent reasoning engine |
| `POST /api/internal/context` | Context snapshot assembly |
| `POST /api/strategies/preview` | Preview signal strategy impact |
| `GET/POST/PUT/DELETE /api/rules` | Filter/scoring rule CRUD |

The frontend-side authoritative contract for these endpoints (DTOs, pagination, null-safety) lives in the frontend repo: `docs/api-contract.md` (§11/§12 Explicit API Contract). The backend audits its DTOs against it during P2–P5.

## CI/CD

Push to `master` → GitHub Actions. Three gates run independently:

1. **`lint.yml`** (PR): `cargo-deny` bans (store/vectorize/embedding/event-store/object-store banned from new deps outside infra/delivery) → layered-deps script → `cargo fmt --check` → `cargo clippy -- -D warnings` → **wasm32 check** (`cargo check --target wasm32-unknown-unknown`) → `cargo test --workspace`
2. **`coverage.yml`** (PR): `cargo-llvm-cov` over the 14-crate pure-logic + application set, **`--fail-under-lines 70` hard gate** (current 73.84%), lcov report uploaded
3. **`deploy.yml`** (push to master): wasm check → `worker-build --release` → `npx wrangler deploy` → smoke tests (health + semantic search)

Secrets: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`

## Migration Status

Sprint 6.5 decoupling (Store god-object demolition) is **in progress**. Status as of 2026-08-21:

**Done:** P1 (dependency bans + layered-deps gate) · T1 (baseline: fmt/clippy green) · T2 (infrastructure adapter tests) · T3 (shared-kernel/events contract tests) · T4 (llvm-cov + 70% gate) · T5 (PR wasm gate) · T10 (baseline tracking)

**Remaining:** P2 (domain-owned repository traits for Intelligence/Reflection/Memory) → P3 (adapter migration to `infrastructure`) → P4 (shrink `StoreBackend`, deprecated thin layer with hard TTL) → P5 (application becomes the sole use-case entry) → P6 (delete old crates + `StoreBackend`) → P7 (architecture guardrail `check-architecture.sh`); tests T6 (application use-case tests), T7 (decoupling per-commit guard), T8 (cross-domain integration: observe→claim→signal→decision→reflection), T9 (delivery-layer tests)

Plans: `docs/superpowers/plans/2026-08-21-architecture-decoupling-plan.md` (P1–P7) and `docs/superpowers/plans/2026-08-21-testing-plan.md` (T1–T10).

## Frontend

[intel.getsulix.com](https://intel.getsulix.com) — Astro 5 frontend deployed as a Cloudflare Worker with service binding. Features intelligence radar, signal investigation, decision tracking, trust center, source provenance, semantic search, dark mode, feed management, and cognitive graph.

Repo: [weixc0856-cell/Intel-Web](https://github.com/weixc0856-cell/Intel-Web)

## License

MIT
