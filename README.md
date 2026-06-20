# Sulix Intelligence

> **Personal AI strategic intelligence assistant for indie founders and solo developers.**

Daily automatically generated intelligence briefings, written directly to your Obsidian vault or any Markdown directory.

## Pipeline

```
RSS feeds → Concurrent fetch → SQLite dedup → LLM analysis → Markdown briefing → Your vault
             (feed-rs)           (rusqlite)       (DeepSeek)                      (DailyBrief/)
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
| Markdown daily briefing generation | ✅ |
| Cron scheduling | ✅ |
| Red-Blue team analysis | 📋 Planned |
| Calibration agent | 📋 Planned |
| Event decay & memory graveyard | 📋 Planned |

## Architecture

```
src/
├── main.rs        # Pipeline orchestration
├── config.rs      # TOML config loader
├── db.rs          # SQLite dedup & storage
├── fetcher.rs     # Concurrent RSS fetching + full-text extraction
├── llm.rs         # DeepSeek API calling with batching & retry
└── renderer.rs    # Markdown briefing renderer
```

The intelligence is driven by **Lens Library** — domain-specific judgment frameworks encoded as system prompts. The core differentiation is not the code, but the cognitive frameworks you inject into each vertical's analysis prompt.

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
