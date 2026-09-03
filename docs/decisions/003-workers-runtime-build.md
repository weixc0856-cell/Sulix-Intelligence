# ADR-003: Workers Runtime & Build Governance

**Date:** 2026-09-03
**Status:** Accepted

## Context

Sulix ships a 27-member Rust workspace whose `worker-entry` crate compiles to
`wasm32-unknown-unknown` via `worker-build` and deploys through Wrangler to a
Cloudflare Worker (D1 / Queue / Scheduled / Vectorize / R2 / AI / service-bound
to the Astro frontend). The overall architecture is sound, but the build chain,
Cloudflare config, and toolchain were drifting:

- `migrations_dir` was declared under `[vars]` (parsed as an env var, so D1
  migrations were never applied from config).
- Worker build was invoked manually in CI and had no `[build]` entry point, so
  `wrangler dev` locally and `wrangler deploy` in CI could build differently.
- `worker-build` and the Rust toolchain were unpinned (floating `stable`).
- Two copies of the D1 migrations existed (root + stale `worker-entry/` copy).

We reviewed Cloudflare's `workers-rs` monorepo
(`D:\Project\RUST_Study\workers-rs-main`) as a **reference baseline** for its
build conventions.

## Decision

Adopt upstream-compatible build conventions **where they solve a Sulix problem**;
do not copy workers-rs config wholesale. Concretely:

1. **Wrangler `[build]` is the single Worker build entry.** `wrangler dev` and
   `wrangler deploy` both run `worker-build --release` via
   `[build] command` in `crates/worker-entry/wrangler.toml`. The `[build]`
   command does **not** `cargo install` — toolchain installation belongs to
   `rust-toolchain.toml` + the CI cache, not to a per-run build hook.
2. **`migrations_dir` belongs inside `[[d1_databases]]`**, not `[vars]`.
   Root `migrations/` (0001…0049) is the single source of truth; the Worker
   crate owns no migrations.
3. **Toolchain pinned by `rust-toolchain.toml`** (channel 1.97.0, profile
   minimal, components rustfmt+clippy, target wasm32-unknown-unknown). CI
   `dtolnay` pins match it. Rationale: reproducible wasm builds; 1.97.0 was
   validated across native + wasm + worker-build before pinning.
4. **Exact `worker-build@0.8.5`** in CI. Rust 0.x `^0.8` ranges are not safe
   ranges; exact pin + cache for reproducibility.
5. **Wasm is a deploy gate.** `deploy.yml` runs
   `cargo check --workspace --all-features --target wasm32-unknown-unknown`
   before deploy (the real artifact target), in addition to the PR gate.

### Explicitly NOT copied from workers-rs

- **No `.cargo/config.toml` with `--cfg getrandom_backend="wasm_js"`.**
  Verified: getrandom is **absent from Sulix's wasm32 graph** — `uuid 1.24`
  routes its RNG through its own js-sys backend on wasm (its getrandom
  dependency is declared non-wasm-only), and getrandom 0.4.3 selects the wasm
  backend via the `wasm_js` **cargo feature**, not via that rustflags cfg.
  The workers-rs cfg works there because their tree sits on getrandom 0.3.
  Copying it here would be inert. The load-bearing mechanism stays the
  workspace-root `uuid = { version = "1", features = ["js"] }`.
- **No blanket `crate-type = ["cdylib", "rlib"]`.** `cdylib` is what the wasm
  artifact requires; `worker-entry` keeps `["cdylib"]` unless a native
  rlib consumer appears.

## Scope

- Worker entry crate, wasm target, `worker-build`, Wrangler config, D1
  migration configuration, Rust toolchain, CI deployment steps, repo hygiene
  (migrations single-source, `build/` ignored).

## Non-goals

- DDD decoupling P0–P7 (`docs/architecture/final-architecture-v2.md`).
- Domain / application / infrastructure re-partitioning, service splits,
  API / Queue / D1 schema redesign.
- Frontend `intel-web`.

These stay separate workstreams and must not ride along on build-governance
changes.

## Consequences

- One build path locally and in CI; `wrangler deploy` reproduces `wrangler dev`.
- D1 migrations are discoverable/appliable from config (verified
  `--local`; remote state must be confirmed in CI via
  `wrangler d1 migrations list --remote`).
- Pinned toolchain + worker-build → reproducible deployments; a future bump is
  an explicit, reviewed change (validate then update in the three CI workflows
  plus `rust-toolchain.toml`).
- If a future dependency introduces getrandom 0.4 into the wasm graph, enable
  its `wasm_js` **feature** explicitly — do not reach for the 0.3-era cfg.
