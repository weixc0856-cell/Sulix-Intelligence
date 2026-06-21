<p align="center">
  <a href="README.md">🇬🇧 English</a> · <a href="README.zh-CN.md">🇨🇳 中文</a>
</p>

# Sulix Intelligence

> **Personal AI strategic intelligence assistant for indie founders and solo developers.**

Daily automatically generated intelligence briefings, written to your Obsidian vault or deployed as a static HTML briefing to Cloudflare Pages.

## Pipeline

```
RSS / YouTube / Wikipedia → Concurrent fetch → Delta dedup (title similarity)
       (feed-rs)               (rusqlite)         (Jaccard bigram 0.75)
                                    │
                        ┌───────────┴───────────┐
                        ▼                       ▼
                Wikipedia context             Keyword filter
                (zh → en fallback)            (for high-throughput 财联社)
                                    │
                                    ▼
                            SQLite dedup
                            (by URL hash)
                                    │
                                    ▼
                        ┌─── [Phase A] Scan Agent ───┐
                        │  (lightweight LLM filter)   │
                        └─────────────────────────────┘
                                    │
                                    ▼
                        ┌─── [Phase B] Red-Blue Team ──┐
                        │  🔴 Synthesis (opportunity)  │
                        │  🔵 Verification (risk audit) │
                        │  ⚖️  Orchestrator (arbitration)│
                        └──────────────────────────────┘
                                    │
                                    ▼
                        [Phase C] Calibration Agent
                        (cognitive bias probing)
                                    │
                                    ▼
              ┌─────────────────────┴─────────────────────┐
              ▼                                           ▼
    Markdown → Obsidian vault           HTML → Cloudflare Pages
    (DailyBrief/YYYY-MM-DD.md)          (DailyBrief/index.html)
                                    │
                                    ▼
                        [Phase D] Decay Agent
                        (memory graveyard)
```

## Tech Stack

`Rust` + `feed-rs` + `scraper` + `reqwest` + `tokio` + `rusqlite` + `DeepSeek API` + `Wikipedia API` + `HTML/Tailwind` + `Cloudflare Pages`

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
# Output: DailyBrief/YYYY-MM-DD.md (Markdown for vault)
#         DailyBrief/index.html (Tailwind HTML for hosting)
```

## Features

| Feature | Status |
|---------|--------|
| RSS/Atom/JSON Feed + YouTube RSS fetching | ✅ |
| Full-text extraction (scraper) | ✅ |
| **Delta dedup** — Jaccard bigram similarity merge multi-source same story | ✅ |
| SQLite dedup & storage | ✅ |
| **Wikipedia context injection** — auto-fetch zh/en summaries for tech terms | ✅ |
| **Keyword pre-filter** — regex whitelist for high-throughput 财联社 sources | ✅ |
| LLM analysis (DeepSeek) with batching & retry | ✅ |
| **Scan Agent** — pre-filter noise/ads before analysis | ✅ |
| **Red-Blue team** — opportunity scout + risk auditor + arbitration | ✅ |
| **Strategic level (S/A/B/C)** — paradigm shift / quarterly impact / watch / noise | ✅ |
| **De-AI-fied McKinsey-style writing** — banned fluff words, verb-driven | ✅ |
| **Calibration Agent** — cognitive bias probing (1 question per day) | ✅ |
| **Decay Agent** — memory graveyard with wake signals | ✅ |
| **HTML static page** — Tailwind CSS, Cloudflare-ready | ✅ |
| Markdown daily briefing generation | ✅ |
| Cron scheduling | ✅ |

## Architecture

```
src/
├── main.rs              # Pipeline orchestration (Phase A→B→C→D)
├── config.rs            # TOML config loader
├── db.rs                # SQLite dedup, storage & graveyard queries
├── fetcher.rs           # Concurrent RSS/YouTube fetching + full-text + keyword filter + delta dedup
├── enricher.rs          # Wikipedia API context injection (zh → en fallback)
├── llm.rs               # DeepSeek API calling with batching & retry
├── renderer.rs          # Markdown + Tailwind HTML briefing renderer
└── agent/
    ├── mod.rs           # Module declaration
    ├── scan.rs          # [Phase A] Scan Agent — fast pre-filter
    ├── synthesis.rs     # [Phase B] 🔴 Red team — opportunity scout
    ├── verification.rs  # [Phase B] 🔵 Blue team — risk auditor
    ├── orchestrator.rs  # [Phase B] ⚖️  Arbiter — per-article arbitration
    ├── calibration.rs   # [Phase C] 🤖 Calibration — cognitive bias questions
    └── decay.rs         # [Phase D] 🪦 Decay Agent — memory graveyard
```

## Output Format

When Red-Blue mode is active, each article is rendered as a decision card:

```
📌 今日核心信号

**Title** — 重要性:8/10 | 战略:A | 信心:L4
💬 One-line plain-summary (≤40 chars)

🔴 红军: Business opportunity — who benefits, why now (≤60 chars)
🔵 蓝军: Execution risk — hidden costs, evidence level (≤60 chars)
⚖️ 仲裁: Per-article arbitration conclusion
🎯 我的判断: Specific advice for the founder

---

<details>📦 其他信号 (N 条)...</details>

🤖 认知校准
```

## Judgment Framework

Every article is evaluated with a founder-first lens:

| Dimension | Scale |
|-----------|-------|
| Strategic Level | S / A / B / C (paradigm shift / quarterly impact / watch / noise) |
| Importance | 1-10 |
| Evidence Level | L1 (proof) - L5 (marketing hype) |
| Actionability | Act Now / Research / Observe / Ignore |

## Writing Style (De-AI-fied)

Output follows McKinsey/Goldman professional services standards:
- **Verb-driven**: hard data + strong verbs, zero adjectives
- **BLUF**: conclusion first, always
- **Red/Blue**: ≤60 chars each, no fluff
- **Banned words**: 惊人, 炸裂, 不可否认, 双刃剑, 值得注意的是, 总而言之, 时代的浪潮

## Configuration

`config.toml` is the brain of the system. Key sections:

- `[llm]` — API key, model, endpoint
- `[[sources]]` — RSS feeds, each with name, URL, category, layer
- `[prompts]` — Base + per-vertical system prompts (your competitive edge)
- `[prompts.vertical_overrides]` — Domain-specific frameworks: AI, 技术主线, 创业, A股, 芯片, 政策
- `[scan_agent]` — Phase A: enable/disable, importance threshold
- `[agent]` — Phase B: enable/disable Synthesis and Verification
- `[graveyard]` — Phase D: retention days, compression, burial threshold

### Source Layers

| Layer | Name | Description |
|-------|------|-------------|
| 1 | Signal Source | Official blogs, Wikipedia API, YouTube tech channels |
| 2 | Curated Source | Pre-filtered by humans, highest signal quality |
| 3 | Community Source | HN, Reddit — alpha before media picks up |
| 4 | Market Source | GitHub Trending, funding data |

## Deployment

Generate a static HTML briefing and deploy to Cloudflare Pages:

```bash
cargo run --release
# Output: DailyBrief/index.html → CF Pages
# Zero server cost, global CDN, no ICP备案 needed
```

## License

MIT
