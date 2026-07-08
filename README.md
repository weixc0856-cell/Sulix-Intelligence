<p align="center">
  <a href="README.md">🇬🇧 English</a>
</p>

# Sulix Intelligence

> **Fully Automated Cognitive Engine — Personal Strategy OS for solo entrepreneurs.**

Sulix Intelligence transforms raw signals into structured **strategic memory** — Signal → Assessment → Decision → Outcome. Not "what happened", but "does this change my decision for the next 6 months."

```
Raw Signals (RSS/USPTO/Reddit)
    ↓
Pipeline → Scan Agent → Theme Clustering
    ↓
Cognitive Engines (Memory + Hermes + Decision)
    ↓
ArtifactSet (Signals / Assessments / Decisions / Outcomes)
    ↓
Schema Validation Gate (reject incomplete objects)
    ↓
Local Storage + R2 (immutable assets) + Frontend Sync
    ↓
MDX View (derived from JSON artifacts)
```

## Architecture

```
                    sulix-engine (Rust)
                           |
                    ArtifactSet JSON
                  ┌─────────┼─────────┐
                  ↓         ↓         ↓
                 R2        D1       Frontend
              (assets)  (index)    (Astro UI)
                           |
                    Cloudflare Worker
                     JSON API Layer
                           |
                    Astro UI Shell
                  (Bloomberg Terminal)
```

## Three-Repository Architecture

| Repo | Responsibility | Tech Stack |
|------|--------------|------------|
| **sulix-engine** ← this repo | Data Acquisition, Analysis, Strategic Memory | Rust + DeepSeek API |
| [sulix-web](https://github.com/weixc0856-cell/Intel-Web) | UI Shell, Navigation, UX | Astro + Tailwind |
| **sulix-docs** | Product Decisions, Architecture, ADR | Obsidian Markdown |

## Products

| Product | Purpose | Price |
|---------|---------|-------|
| **News Layer** | User acquisition | $0 |
| **Research Layer** | Revenue | $99-$4999 |
| **Memory Layer** | Moat | Private |

## Quick Start

```bash
# 1. Build
git clone https://github.com/weixc0856-cell/Sulix-Intelligence.git
cd Sulix-Intelligence
cp config.example.toml config.toml
cargo build --release

# 2. Run (requires DEEPSEEK_API_KEY)
export DEEPSEEK_API_KEY="sk-..."
cargo run --release

# 3. Preview
cd ../sulix-web
cp -r ../Sulix-Intelligence/output/* src/content/
npm install && npm run dev
```

## Pipeline

```
Source Acquisition (RSS / USPTO / Reddit)
    ↓ Pipeline: sanitize → compliance → dedup
    ↓ Evidence Snapshot (immutable JSONL, SVI ≥ 5)
    ↓ Scan Agent v1.1 (3-tier: Insight / Watchlist / Signal Memory)
    ↓ LLM Pre-dedup → Theme Clustering (≤5 themes)
    ↓ Theme Analysis + ASI/Confidence Scoring
    ↓ Blue Team Verification (load-bearing assumption challenges)
    ↓ Editor Agent (personal impact analysis)
    ↓ MemoryEngine (Thesis / Evidence / Outcome / Reflection)
    ↓ Hermes (Change Detection / Trends / Conflicts)
    ↓ Decision Intelligence (Thesis → Decision mapping)
    ↓ Meta Layer (auto Outcome detection + Reflection generation)
    ↓ validation gate (schema::validator)
    ↓ Artifact Publisher → Local + R2 + Frontend Sync
    ↓ Event Log flush (data/events/{date}.jsonl)
```

## Code Structure

```
src/
├── domain/           — 8 domain models (Theme/Thesis/Evidence/Action/Outcome/Reflection/Decision/ArtifactSet)
├── engine/           — Cognitive engines (analysis/memory/premium/belief/decision)
├── publishing/       — 5-stage publish coordinator → returns ArtifactSet
├── artifact/         — Manifest, Report, Builder (pure functions)
├── delivery/         — Validation gate → Local → R2 → Frontend sync + Event flush
├── schema/           — Schema validation (schemars derive + Validate trait)
├── storage/          — R2 upload client (S3-compatible), corrupt-recovery helpers
├── renderer/         — MDX/Markdown/HTML rendering (MDX derived from JSON)
├── hermes/           — Change detection + trends + conflicts
├── clusterer/        — Theme clustering + LLM pre-dedup + synthesis
├── agent/            — Scan Agent + Editor Agent + Calibration + Decay
├── source/           — Source adapters (RSS/USPTO/Reddit)
├── event_log/        — ObjectEvent audit trail (append-only JSONL)
├── main.rs           — Pipeline orchestration (558 lines)
└── lib.rs            — Module declarations
```

## Schema Validation Gate

Every artifact passes validation before storage. Rejected objects go to `data/rejected/{date}/` and trigger non-zero exit.

| Check | Phase 0 | Phase 1 |
|-------|---------|---------|
| Required fields non-empty | ✅ | ✅ |
| Confidence in [0,1] | ✅ | ✅ |
| Evidence array non-empty | ⚠️ warning | ❌ reject |
| Decision type in enum | ✅ | ✅ |

## Events

All object lifecycle events are recorded in `data/events/{date}.jsonl`:

```json
{"schema_version":1,"event_type":"decision_created","object_id":"DEC-0001","summary":{"confidence":0.72}}
{"schema_version":1,"event_type":"outcome_recorded","object_id":"OUT-001","summary":{"verdict":"PartiallyConfirmed"}}
{"schema_version":1,"event_type":"publish_completed","summary":{"passed":3,"rejected":0,"r2_status":"not_configured"}}
```

Events contain summaries only (not full snapshots). Full object history in R2.

## Configuration

| Section | Purpose |
|---------|---------|
| `[llm]` | API key, model, endpoint |
| `[[sources]]` | Data sources (name, URL, category, layer, score) |
| `[prompts]` | System prompts for each analysis stage |
| `[output]` | Output paths (vault_path, mdx_dir, frontend_public_dir) |
| `[storage]` | data_dir for persistent state |
| `[r2]` | Cloudflare R2 config (bucket, endpoint, public_url) |

## Deployment

### CI Pipeline (GitHub Actions)

```yaml
# .github/workflows/cron_brief.yml
# Daily: cargo run --release → R2 → sulix-web build → CF Pages
```

Secrets required:
- `DEEPSEEK_API_KEY` — LLM provider
- `R2_ACCESS_KEY_ID` / `R2_SECRET_ACCESS_KEY` / `R2_ENDPOINT` — R2 storage
- `CLOUDFLARE_API_TOKEN` / `CLOUDFLARE_ACCOUNT_ID` — Pages deploy

## License

MIT
