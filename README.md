<p align="center">
  <a href="README.md">🇬🇧 English</a> · <a href="README.zh-CN.md">🇨🇳 中文</a>
</p>

<p align="center">
  <img src="assets/logo.svg" width="120" alt="Sulix Intelligence" />
</p>

# Sulix Intelligence

> **Fully Automated AI Think Tank — Personal Strategy OS for solo entrepreneurs.**

Sulix Intelligence is a **cognitive engine** that processes raw signals into structured knowledge assets:

```
Raw Signals → Cognitive Engine → MDX Knowledge Assets → Astro Frontend (separate repo)
```

**Answer:** Not "what happened" — but "does this change my decision for the next 6 months."

## Architecture

```
                              Sulix-Intelligence (Engine)
                                      │
                          Rust Pipeline (Cognitive Engine)
                          LLM Analysis + SVI/ASI + MemoryEngine
                                      │
                                  MDX Output
                          output/{daily,thesis,research,memory}/
                                      │
                          GitHub Action (copy + commit)
                                      │
                              Intel-Web (Frontend)
                          Astro + Content Collections + Tailwind
                          intel.getsulix.com
```

### Three Products

| Product | Purpose | Format | Price |
|---------|---------|--------|-------|
| **News Layer** | User acquisition | Daily MDX signals | $0 |
| **Research Layer** | Revenue | Multi-agent reports · MDX | $99-$4999 |
| **Memory Layer** | Moat | Thesis tracking · MDX | Private |

### Tech Stack

| Layer | Stack |
|-------|-------|
| Engine | Rust + feed-rs + scraper + reqwest + tokio + rusqlite |
| LLM | DeepSeek / OpenAI API (BYOK) |
| Knowledge Format | MDX (YAML frontmatter + Markdown) |
| Frontend | Astro + TypeScript + Tailwind (in [Intel-Web](https://github.com/weixc0856-cell/Intel-Web)) |
| Deploy | Cloudflare Pages + GitHub Actions |
| Cost | ~$0/mo infrastructure + LLM API (~$3/mo) |

## Quick Start

```bash
# 1. Clone and build
git clone https://github.com/weixc0856-cell/Sulix-Intelligence.git
cd Sulix-Intelligence
cargo build --release

# 2. Configure
cp config.example.toml config.toml
# Set your DeepSeek API key in [llm] section
# Set mdx_dir = "output" in [output] section

# 3. Run
cargo run --release

# Output:
#   output/daily/YYYY-MM-DD-slug.mdx    → Daily signals
#   output/thesis/YYYY-MM-DD-slug.mdx   → Thesis tracking
#   output/research/YYYY-MM-DD-slug.mdx → Premium reports
#   data/belief_db.json                 → Memory Layer

# 4. Preview with frontend
git clone https://github.com/weixc0856-cell/Intel-Web.git
cd Intel-Web
cp -r ../Sulix-Intelligence/output/* src/content/
npm install && npm run dev  # → http://localhost:4321
```

## Pipeline

```
RSS/USPTO Sources → RawSignal → Pipeline (sanitize + compliance + dedup)
  ↓
Evidence Snapshot (SVI ≥ 5 → immutable JSONL evidence log)
  ↓
Wikipedia Enrichment + Full-Text Extraction
  ↓
EntitySanctionDb Extraction
  ↓
Scan Agent v1.1 (4-class tags, 3-tier triage: Insight/Watchlist/Memory)
  ↓
LLM Pre-dedup → Theme Clustering (≤5 themes)
  ↓
Theme Analysis (BLUF / Impact / Geopolitical / Supply Chain / Causal Chains)
  ↓
ASI + Confidence Scoring
  ↓
Blue Team Verification (load-bearing assumption challenge)
  ↓
DiGraph Cognitive Engine (QE → Belief Engine → Decision Engine)
  ↓
Editor Agent (personal impact analysis)
  ↓
Change Detection + Trend Layer
  ↓
MemoryEngine (Thesis + Evidence + Outcome + Reflection)
  ↓
MDX Output: daily/  thesis/  research/  memory/
```

## MDX Knowledge Format

Sulix generates MDX files with YAML frontmatter. Example `output/daily/2026-06-24-ai-agent.mdx`:

```mdx
---
title: AI Agent Infrastructure Consolidation
date: 2026-06-24
svi: 8.7
asi: 7.5
confidence: 0.81
type: daily
sources: [Federal Register, SEC]
entities: [TSMC, NVIDIA]
---

## BLUF

One-sentence bottom line.

## Analysis

Detailed analysis with impact, geopolitical, supply chain.

## Evidence

| Evidence | Interpretation | Confidence |
|----------|---------------|------------|

## Assumptions

- 🔴 Assumption text (evidence strength: weak)

## Personal Impact

- 👀 Q1: Strengthens your "build apps" thesis (+2) [Explore]
```

This format is:
- **Git-friendly** — diff, review, history
- **Astro-native** — `getCollection("daily")` directly
- **Human-readable** — edit in any text editor

## Features

| Feature | Status |
|---------|--------|
| 29 data sources with Source Scoring | ✅ |
| SVI Strategic Volatility Index | ✅ |
| ASI + Confidence scoring | ✅ |
| Scan Agent 3-tier triage | ✅ |
| Editor Agent (Personal Impact) | ✅ |
| Blue Team verification | ✅ |
| DiGraph Cognitive Engine | ✅ |
| MemoryEngine (Thesis + Outcome + Reflection) | ✅ |
| Belief Engine Phase B (WayneOPC) | ✅ |
| Meta Layer (auto outcome detection) | ✅ |
| MDX knowledge output | ✅ |
| Twitter/X tweet pipeline | ✅ |
| Reddit data source | ✅ |
| Change Detection (rule + LLM) | ✅ |
| Event Log (append-only audit trail) | ✅ |
| Chronicle (history database) | ✅ |
| Bilingual EN/ZH | ✅ |
| LLM Audit counters | ✅ |
| Substack API integration | ✅ |

### Code Structure (59+ files)

```
src/
├── domain/        — 7 domain models (Theme/Thesis/Evidence/Observation/Action/Outcome/Reflection)
├── engine/        — Domain engines (analysis/memory/premium/belief)
├── hermes/        — Change detection + trends + conflicts
├── renderer/      — MDX output + HTML (debug) + Markdown
├── clusterer/     — Theme clustering + synthesis
├── agent/         — Scan Agent + Editor Agent + Calibration + Decay
├── source/        — Source adapters (RSS/USPTO/Reddit)
├── twitter.rs     — X/Twitter auto-tweet pipeline
├── publishing.rs  — Publishing agent orchestration
├── event_log.rs   — PipelineEvent audit log
└── main.rs        — Pipeline orchestration (629 lines)
```

## Configuration

| Section | Purpose |
|---------|---------|
| `[llm]` | API key, model, endpoint |
| `[[sources]]` | Data sources with name, URL, category, layer, score |
| `[prompts]` | System prompts for each analysis stage |
| `[output]` | Output paths, including `mdx_dir` |
| `[questions]` | Active decision questions |
| `[beliefs]` | WayneOPC core beliefs (B1-B10) |
| `[twitter]` | X/Twitter API config |
| `[graveyard]` | Decay Agent settings |

## Deployment

### Pipeline (cron)

```bash
# Daily (Linux/macOS)
0 6 * * * cd /path/to/Sulix-Intelligence && cargo run --release

# Daily (Windows)
cargo run --release
```

### Frontend

Frontend is a separate repo: [Intel-Web](https://github.com/weixc0856-cell/Intel-Web)

```bash
git clone https://github.com/weixc0856-cell/Intel-Web.git
cd Intel-Web
cp -r ../Sulix-Intelligence/output/* src/content/
npm install && npm run build
```

## License

MIT
