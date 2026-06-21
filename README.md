<p align="center">
  <a href="README.md">🇬🇧 English</a> · <a href="README.zh-CN.md">🇨🇳 中文</a>
</p>

# Sulix Intelligence

> **Personal AI strategic intelligence assistant for indie founders and solo developers.**

Daily automatically generated intelligence briefings, written directly to your Obsidian vault or any Markdown directory.

## Pipeline

```
RSS feeds → Concurrent fetch → SQLite dedup → Full-text extraction → Group by category
             (feed-rs)           (rusqlite)     (scraper)
                              │
                  ┌───────────┴───────────┐
                  ▼                       ▼
          [Phase A] Scan Agent    (skip noise/ads)
                  │
                  ▼
          [Phase B] Red-Blue Team
             ├─ 🔴 Synthesis (optimist narrative)
             ├─ 🔵 Verification (skeptic rebuttal)
             └─ ⚖️  Orchestrator (arbitration)
                  │
                  ▼
          [Phase C] Calibration Agent (bias probing)
                  │
                  ▼
          Markdown briefing → DailyBrief/ (with debate traces)
                  │
                  ▼
          [Phase D] Decay Agent (memory graveyard)
             ├─ Bury old/stale articles
             └─ Wake signals on re-emerging topics
```

## Tech Stack

`Rust` + `feed-rs` + `scraper` + `reqwest` + `tokio` + `rusqlite` + `DeepSeek API` + `Markdown` + `Cron`

## Quick Start

```bash
# 1. Clone and build
git clone https://github.com/weixc0856-cell/Sulix-Intelligence.git
cd Sulix-Intelligence
cargo build --release

# 2. Configure
cp config.example.toml config.toml
# Edit config.toml — set your DeepSeek API key and RSS sources

# 3. Run
cargo run --release
```

## Features

| Feature | Status |
|---------|--------|
| RSS/Atom/JSON Feed fetching | ✅ |
| Full-text extraction (scraper) | ✅ |
| SQLite dedup & storage | ✅ |
| LLM analysis (DeepSeek) with batching | ✅ |
| Retry with exponential backoff | ✅ |
| **Scan Agent** — pre-filter noise/ads before analysis | ✅ |
| **Red-Blue team** — optimistic narrative + skeptical rebuttal + arbitration | ✅ |
| **Calibration Agent** — cognitive bias probing (1 question per day) | ✅ |
| **Decay Agent** — memory graveyard with wake signals | ✅ |
| Markdown daily briefing generation | ✅ |
| Cron scheduling | ✅ |

## Architecture

```
src/
├── main.rs              # Pipeline orchestration (Phase A→B→C→D)
├── config.rs            # TOML config loader
├── db.rs                # SQLite dedup, storage & graveyard queries
├── fetcher.rs           # Concurrent RSS fetching + full-text extraction
├── llm.rs               # DeepSeek API calling with batching & retry
├── renderer.rs          # Markdown briefing renderer
└── agent/
    ├── mod.rs           # Module declaration
    ├── scan.rs          # [Phase A] Scan Agent — fast pre-filter
    ├── synthesis.rs     # [Phase B] 🔴 Red team — optimistic narrative
    ├── verification.rs  # [Phase B] 🔵 Blue team — skeptical rebuttal
    ├── orchestrator.rs  # [Phase B] ⚖️  Arbiter — merge Red+Blue
    ├── calibration.rs   # [Phase C] 🤖 Calibration — cognitive bias questions
    └── decay.rs         # [Phase D] 🪦 Decay Agent — memory graveyard
```

The intelligence is driven by **Lens Library** — domain-specific judgment frameworks encoded as system prompts. The core differentiation is not the code, but the cognitive frameworks you inject into each vertical's analysis prompt.

## Agent Pipeline

The pipeline runs 4 agent phases after articles are fetched and grouped:

**Phase A — Scan Agent.** A lightweight LLM call per article batch that scores importance (1-10). Articles below the threshold (default ≤3) are skipped as noise/PR/advertising. Saves token cost by filtering before deep analysis.

**Phase B — Red-Blue Team.** Two independent LLM passes with opposing roles:
- 🔴 **Synthesis** (Red): Optimistic narrative builder. Connects dots across sources, identifies trends, spots opportunities.
- 🔵 **Verification** (Blue): Extreme skeptic. Applies evidence-level ratings (L1-L5) and the "AI Myth Busting Six Questions" to challenge every claim.
- ⚖️ **Orchestrator**: Pure logic (no LLM). Merges Red+Blue outputs, flags L4/L5 warnings, signals consensus at L1/L2.

**Phase C — Calibration Agent.** One pointed question per day appended to the briefing. Probes cognitive blind spots and contradictions in the day's analysis. Designed to make you think, not to provide answers.

**Phase D — Decay Agent.** Background maintenance after the briefing is written: buries articles past their retention window (default 90 days), optionally compresses them via LLM, and checks if any newly-arriving article matches a previously buried topic — if so, surfaces a "wake signal" in the briefing.

## Judgment Framework

Every article is evaluated across 5 dimensions:

| Dimension | Scale |
|-----------|-------|
| Importance | 1-10 |
| Relevance | High / Medium / Low |
| Time Horizon | Short-term / Mid-term / Long-term |
| Actionability | Act Now / Research / Observe / Ignore |
| Confidence | High / Medium / Low |

## Configuration

`config.toml` is the brain of the system. Key sections:

- `[llm]` — API key, model, endpoint
- `[[sources]]` — RSS feeds, each with name, URL, category, layer
- `[prompts]` — System prompts per vertical (this is where your edge lives)
- `[scan_agent]` — Phase A: enable/disable, importance threshold
- `[agent]` — Phase B: enable/disable Synthesis and Verification
- `[graveyard]` — Phase D: retention days, compression, burial threshold
- `[storage]` — data directory for SQLite database
- `[output]` — vault path for daily briefings
- `[dedup]` — dedup window and title similarity threshold

### Source Layers

Sources are organized in a 4-layer model:

| Layer | Name | Description |
|-------|------|-------------|
| 1 | Signal Source | Official blogs, most accurate but hardest to read |
| 2 | Curated Source | Pre-filtered by humans, highest signal quality |
| 3 | Community Source | Alpha originates here before media picks up |
| 4 | Market Source | Jobs, funding, open-source trends |

## License

MIT
