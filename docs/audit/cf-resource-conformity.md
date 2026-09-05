# CF Resource Conformity Audit

- **Date**: 2026-09-05
- **Scope**: Read-only audit of the 5 Cloudflare resource classes the backend Worker binds —
  D1 / R2 / Queue / KV / Vectorize — checking each across four layers:
  `wrangler.toml declaration → Rust binding access → runtime error behavior → workers-rs / Cloudflare reference pattern`.
- **Baseline**: repo HEAD `acfaff8` (P6), account `weixc0856@gmail.com`, wrangler 4.104.0.
- **Nature**: observation only. `Code changes = 0`, `Cloudflare mutations = 0`, `Deployment = 0`.
- **Workers compared against**: Cloudflare workers-rs monorepo (reference baseline per `docs/decisions/003-workers-runtime-build.md`).
- Backend config: `crates/worker-entry/wrangler.toml` (single source). Frontend `D:\Project\intel-web\wrangler.toml` (service-bound, out of scope here).

---

## Executive Summary

All five resource classes exist, are correctly bound (config ↔ Rust binding string ↔ live resource match 1:1),
and the deployed schema is fully applied. No binding points at the wrong resource; the denylist of historical
resources is untouched by this product's configuration.

**Conformity verdict by class:**

| Class | Binding | Live resource | Config↔code↔resource | Reference conformity | Notes |
|---|---|---|---|---|---|
| D1 | `DB` | `sulix-feed-db` `42c2dc1c-…` | ✅ | ✅ | 47/47 migrations applied+tracked; `num_tables=0` in `d1 list` is a reporting quirk |
| R2 | `RAW_CONTENT` | `sulix-feed-raw` | ✅ | ✅ binding | ⚠️ `.ok()` silent degradation at many call sites |
| Queue | `FETCH_QUEUE` | `sulix-feed-fetch-queue` + `-dlq` | ✅ | ✅ | consumer present; DLQ wired |
| KV | `CACHE` | `sulix-feed-cache` `50437e8c-…` | ✅ | ⚠️ | name not declared in config (id only); title confirmed `sulix-feed-cache` |
| Vectorize | `VECTORIZE` | `sulix-article-embeddings` 1024/cosine | ✅ | ⚠️ | custom wasm extern type (`VectorizeIndex`), not a workers-rs native binding; accessed via `env.get_binding` |
| AI | `AI` + vars | Workers AI + DeepSeek chat | ✅ | n/a (dual channel) | architecture note, not a conformity defect |

All deviations below are **recommendations only**. Per the governing plan, none are fixed in this round.

---

## D1

- **Declaration** (`crates/worker-entry/wrangler.toml`): `[[d1_databases]] binding="DB"`,
  `database_name="sulix-feed-db"`, `database_id="42c2dc1c-d385-4f62-9bbf-d5bdb684cd28"`,
  `migrations_dir="../../migrations"` (correct placement — ADR-003).
- **Live**: `wrangler d1 list` → `sulix-feed-db` `42c2dc1c-…` exists (created 2026-09-03T05:20:25Z).
- **Rust access**: pervasive `env.d1("DB")` (api, worker-entry jobs/runtime). Binding string matches.
- **Schema / migration state** (queried live, read-only):
  - `SELECT count(*) FROM sqlite_master WHERE type='table'` → **50** (48 domain tables + FTS shadow tables + `d1_migrations` + `_cf_KV`).
  - `SELECT count(*) FROM d1_migrations` → **47** rows, `0001_init.sql` → `0049_reasoning_frameworks.sql`.
  - Disk `migrations/*.sql` → **47** files (numbering 0001–0049 has gaps — no 0029/0030/0039).
  - `wrangler d1 migrations list --remote` → "No migrations to apply" (= all recorded; wrangler wording).
  - **Conclusion: schema fully applied via the tracked migration flow; 47 = 47.**
- **Finding (audit corrects an earlier suspicion)**: `num_tables=0` in `wrangler d1 list` is **not** evidence of
  an empty/mis-migrated database — every D1 in the account reports 0 (e.g. `rss-db` too). The live DB has 50
  tables. Do not use that column as a health signal.
- **Deviation**: none for binding/schema.
- **Recommendation**: (1) if this audit's purpose is parity with the workers-rs reference, D1 usage matches
  (`env.d1` + `d1_databases` + `migrations_dir`); (2) treat `num_tables` in `d1 list` as unreliable.

---

## R2

- **Declaration**: `[[r2_buckets]] binding="RAW_CONTENT" bucket_name="sulix-feed-raw"`.
- **Live**: bucket exists (created 2026-09-03T05:22:05Z).
- **Rust access**: `env.bucket("RAW_CONTENT")` in api routes (briefing, article, reflection) and worker-entry
  jobs/queue runtime. String matches.
- **Reference pattern**: workers-rs exposes R2 via `env.bucket(binding) -> Result<Bucket>`; this product uses the
  object-store `R2Store` wrapper. Conforms.
- **Deviation / risk**: several call sites discard the `Result` with `.ok()`:
  `env.bucket("RAW_CONTENT").ok()` (see `docs/FULL_REVIEW_REPORT.md:90`). A missing/misnamed bucket degrades to
  `None` → silent runtime degradation (no article body / no raw content) instead of a surfaced error.
- **Recommendation**: convert the silent `.ok()` sites to propagate a binding error (fail fast). **Out of scope this round.**

---

## Queue

- **Declaration**: producer `binding="FETCH_QUEUE" queue="sulix-feed-fetch-queue"`; consumer on
  `sulix-feed-fetch-queue` (`max_batch_size=10, max_batch_timeout=30, max_retries=3, dead_letter_queue="sulix-feed-fetch-dlq"`).
- **Live**: `sulix-feed-fetch-queue` (`0f85ca66…`, created 2026-07-22) producers=1 consumers=1; DLQ
  `sulix-feed-fetch-dlq` (`07159ba1…`, created 2026-09-03) producers=0 consumers=0.
- **Rust access**: `env.queue("FETCH_QUEUE")` (jobs/ingestion). Consumer handler in worker-entry runtime/queue.
- **Conformity**: matches Cloudflare Queues producer/consumer + DLQ pattern. ✅
- **Risk (for the planned reset)**: the main queue has an attached Worker consumer — deletion requires the
  consumer gate (detach/disable → verify → delete), not blind `--force`.
- **Recommendation**: none beyond the reset's consumer gate.

---

## KV

- **Declaration**: `[[kv_namespaces]] binding="CACHE" id="50437e8c1f9b4cf4b8b26c16fda159d0"` — **no name/title field**.
- **Live**: namespace id `50437e8c1f9b4cf4b8b26c16fda159d0`, title **`sulix-feed-cache`** (created 2026-07-22 era per id).
- **Rust access**: `env.kv("CACHE")` (api routes/system + worker-entry jobs). String matches id binding.
- **Deviation**: the config declares only the namespace `id` and not its `title`. `wrangler kv namespace list`
  reports the account title as `sulix-feed-cache`, which matches the `sulix-feed-*` naming — but because the
  config omits the name, the file alone does not document *which* KV it binds. Namespace id + binding are the
  authoritative identity; the title is cosmetic.
- **Recommendation**: add the `title = "sulix-feed-cache"` comment/field to `wrangler.toml` for self-documentation.
  **Out of scope this round.** (Note: KV bindings are by id, not by title — renaming the title does not rebind.)

---

## Vectorize

- **Declaration**: `[[vectorize]] binding="VECTORIZE" index_name="sulix-article-embeddings"`.
- **Live**: index `sulix-article-embeddings`, **1024 dims / cosine**, created 2026-09-03T05:24:08Z.
- **Rust access**: `env.get_binding::<VectorizeIndex>("VECTORIZE")` (api semantic/rebuild, worker-entry
  jobs/ingestion/backfill/signal/queue). Binding name matches.
- **Reference deviation**: `VectorizeIndex` is a **custom wasm-bindgen extern** declared in
  `crates/vectorize/src/lib.rs` (`#[wasm_bindgen] extern "C" { pub type VectorizeIndex; }`,
  `TYPE_NAME = "VectorizeIndexImpl"`). This is not a workers-rs-native binding surface; workers-rs does not ship
  a typed Vectorize API, so access is via the raw binding lookup + hand-written extern. **It works**, but it is a
  deliberate custom shim rather than an upstream idiomatic API. `docs/decisions/002-vector-search.md` documents
  this crate.
- **Risk**: `env.get_binding::<VectorizeIndex>("VECTORIZE").ok()` discards failure at several sites → silent
  degradation if the index binding is missing.
- **Recommendation**: distinguish "works today" from "matches an upstream recommendation": if strict reference
  parity is desired, either pin a workers-rs/extern source of truth for the binding interface, or route Vectorize
  through Cloudflare's HTTP/binding contract with explicit error propagation. **Out of scope this round.**
- **Post-reset note**: index emptiness after reset is expected, not a failure (see plan Part 3.8).

---

## AI Dual Channel (architecture note, not a defect)

- `[ai] binding="AI"` → Workers AI (used by `crates/embedding` for embeddings).
- `[vars] AI_BASE_URL="https://api.deepseek.com/v1"` + `AI_CHAT_MODEL="deepseek-v4-flash"` + secret
  `AI_API_KEY` → DeepSeek-compatible endpoint for chat/summarisation (`crates/worker-entry` services).
- **Observation**: two model providers are in use simultaneously — Workers AI for embeddings, DeepSeek for chat.
  This is recorded architecture state; not changed this round.
- Rust access `env.ai("AI")` (embedding, embedder) and `env.secret("AI_API_KEY")`/`env.var("AI_BASE_URL"|"AI_CHAT_MODEL")`
  match the declarations.

---

## Cross-resource Findings

1. **`.ok()` silent-binding-failure pattern** recurs across R2/Vectorize/KV access (`docs/FULL_REVIEW_REPORT.md:90`).
   Missing/misconfigured bindings degrade silently instead of surfacing. Highest-value non-urgent hardening item.
2. **`num_tables=0` in `wrangler d1 list`** is unreliable (all account DBs report 0); verified live schema is complete.
3. **Resource naming**: all product resources are `sulix-feed-*` except the Vectorize index `sulix-article-embeddings`
   (`sulix-` prefix). Cosmetic inconsistency only; the code refers to each correctly.
4. **Binding↔code↔resource triples all match 1:1** — no cross-wiring between this product and historical resources.
5. Cron flags `CRON_INGESTION_ENABLED` / `CRON_SIGNAL_ENABLED` are declared+read; `CRON_REFLECTION_ENABLED` /
   `CRON_MEMORY_ENABLED` are read in `runtime/cron.rs` but **not declared** in wrangler.toml `[vars]` (feature not
   yet activated — behaves as disabled-by-default).

## Historical Resources

Present in the account but **not referenced by this product's config or code** (identified + documented only;
deletion/rename is prohibited by the governing plan):

- D1: `rss-db` (`6d199e6f-…`), `rss-db-dev` (`3240a792-…`), `sulix-agent-db` (`dee5b298-…`), `sulix-index` (`746af3a9-…`)
- R2: `rss-bucket`, `rss-bucket-dev`, `portal-releases`, `sulix-content`, `sulix-releases`
- KV: `FORM_STORE`, `LICENSE_STORE`, `rss-worker-cache-dev`, `rss-worker-cache-prod`
- Queues: `rss-fetch-queue` (`6176b20e-…`), `rss-fetch-queue-prod` (`6467eba8-…`), `rss-fetch-dlq` (`54e5e51d-…`)
- Workers: `sulix-feed-worker`, `sulix-feed-frontend`, plus historical worker names (`rss-*`, `minimal-sulix-test`)
  live outside the two active configs.

**Recommendation** (future remediation only): account-level cleanup of the historical `rss-*` / `portal-*` /
`agent` / `index` resources is a separate, explicitly-authorized task. Not performed here.

---

## Recommendations (all deferred / out of scope)

1. Harden `.ok()` binding access → explicit binding errors (R2/Vectorize/KV).
2. Add KV namespace `title` to wrangler.toml for self-documentation.
3. Decide Vectorize access target state (custom shim vs upstream binding contract) at architecture level.
4. Declare or remove `CRON_REFLECTION_ENABLED` / `CRON_MEMORY_ENABLED` when those features activate.
5. Account cleanup of historical resources — separate authorized task.

---

## Part 2 Gate

Confirmed: `Code changes = 0`, `Cloudflare mutations = 0`, `Deployment = 0`. This file is documentation only.

---

## Post-Reset Addendum (2026-09-05) — Part 3 Five-Resource Reset

Authorized reset executed 2026-09-05 per the Task B closing plan (Global Safety Contract: Hard
Allowlist enforced — only the 5 `sulix-feed` resource classes; Explicit Denylist untouched).
The audit sections above document the **pre-reset** state at baseline `acfaff8` (P6).

| Resource | Pre-reset id | Post-reset id | Verifications |
|---|---|---|---|
| D1 `sulix-feed-db` | `42c2dc1c-…` | `ee083fd3-7fd2-4571-8d53-5036a263265d` | 47/47 migrations applied; 50 tables (48 domain + FTS shadow + `d1_migrations` + `_cf_KV`) |
| R2 `sulix-feed-raw` | same name | recreated (same name) | exists; `RAW_CONTENT` binding deployed |
| Queue `sulix-feed-fetch-queue` + `-dlq` | `0f85ca66…` / `07159ba1…` | `473b81d3…` / `13e96f54…` | producer + consumer re-attached (1/1); DLQ 0/0 |
| KV `sulix-feed-cache` | `50437e8c…` | `1cdea52318b4401391145b3898f68345` | `CACHE` binding deployed with new id |
| Vectorize `sulix-article-embeddings` | 1024 / cosine | recreated 1024 / cosine | index exists; dims/metric match spec |

- **Deployment**: `wrangler deploy` → version `bc8a2a42-1890-426e-b35e-75cf8c538c92`. Bindings
  verified against the recreated resources (`env.CACHE` = `1cdea523…`, `env.DB` = new `database_id`,
  Vectorize / R2 / Queue bound by name). Producer + consumer for `sulix-feed-fetch-queue` re-attached.
- **Runtime**: `/api/health` → HTTP 200, `status:"ok"` (D1 live; fresh-DB zeros expected).
  `/api/articles/search` mode `semantic` → returns `mode:"semantic"`, `results:[]` (index empty
  post-reset = expected; embedding backfill **out of scope** per plan).
- **Config**: `crates/worker-entry/wrangler.toml` changed only in two lines — D1 `database_id` →
  `ee083fd3-…`, KV `id` → `1cdea523…`.
- **Denylist sanity (post-reset)**: all historical resources unchanged — D1 `rss-db` `6d199e6f-…`,
  `rss-db-dev` `3240a792-…`, `sulix-agent-db` `dee5b298-…`, `sulix-index` `746af3a9-…`; R2
  `rss-bucket`, `rss-bucket-dev`, `portal-releases`, `sulix-content`, `sulix-releases`; KV
  `FORM_STORE`, `LICENSE_STORE`, `rss-worker-cache-dev`, `rss-worker-cache-prod`; queues
  `rss-fetch-dlq` `54e5e51d-…`, `rss-fetch-queue` `6176b20e-…`, `rss-fetch-queue-prod` `6467eba8-…`.
  Ids match the pre-reset snapshot exactly.

Note: sections above reflect the state *before* this addendum's reset and remain a valid read-only
audit of that point in time; post-reset identities are recorded in this addendum.
