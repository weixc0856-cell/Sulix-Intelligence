<p align="center">
  <a href="README.md">🇬🇧 English</a> · <a href="README.zh-CN.md">🇨🇳 中文</a>
</p>

<p align="center">
  <img src="assets/logo.svg" width="120" alt="Sulix Intelligence" />
</p>

# Sulix Intelligence

> **Fully Automated AI Think Tank — Personal Strategy OS for solo entrepreneurs.**

Sulix Intelligence is a three-layer product built on a single Rust pipeline:

```
                 Source Layer (21+ data sources)
                      │
               Signal Layer (SVI + Compliance + Clustering)
                      │
          ┌───────────┼────────────┐
          │           │            │
    News Layer  Research Layer  Memory Layer
    (free)      (paid reports)  (private)
```

- **News Layer** → Bloomberg Terminal-style dashboard. Daily signal aggregation. Free.
- **Research Layer** → Multi-agent deep research reports. $99-$4999. Paid.
- **Memory Layer** → Belief tracking, contradiction detection, decision history. Private.

**Answer:** Not "what happened" — but "does this change my decision for the next 6 months."

## Architecture

```
                               Rust Pipeline
                                    │
                    ┌───────────────┼───────────────┐
                    │               │               │
               Track 1:        Track 2:         Track 3:
                 HTML          Markdown +       BeliefDb JSON
                 (Obsidian)    Frontmatter      (Memory Layer)
                    │               │               │
                    ▼               ▼               │
              DailyBrief/    Astro Frontend          │
              Local view     intel.getsulix.com      │
                                    │               │
                                    ▼               ▼
                              CF Pages          /memory/
                              (public)         Dashboard
```

### Tech Stack

| Layer | Stack |
|-------|-------|
| Backend | Rust + feed-rs + scraper + reqwest + tokio + rusqlite |
| LLM | DeepSeek / OpenAI API (BYOK) |
| Frontend | Astro + TypeScript + JetBrains Mono + Inter |
| Cache | LayeredCache (memory HashMap + TTL) + CircuitBreaker |
| Auth | Substack (Newsletter) + Stripe/LemonSqueezy (Research) |
| Deploy | Cloudflare Pages + GitHub Actions |
| Cost | ~$0/mo infrastructure + LLM API (~$3/mo) |

### Three Products

| Product | Purpose | Format | Price |
|---------|---------|--------|-------|
| **News Layer** | User acquisition | Terminal Dashboard · Daily email | $0 |
| **Research Layer** | Revenue | Multi-agent reports · PDF | $99-$4999 |
| **Memory Layer** | Moat | Belief tracking · Decision history | Private |

## Quick Start

```bash
# 1. Clone and build
git clone https://github.com/weixc0856-cell/Sulix-Intelligence.git
cd Sulix-Intelligence
cargo build --release

# 2. Configure
cp config.example.toml config.toml
# Set your DeepSeek API key and data sources

# 3. Run
cargo run --release

# Output:
#   DailyBrief/en/YYYY-MM/index.html  → News Layer (local view)
#   content/posts/                    → Astro Markdown
#   data/belief_db.json              → Memory Layer

# 4. Build frontend
cd astro-frontend
npm install && npm run build

# 5. Start frontend dev server
npm run dev        # → http://localhost:4321
```

## Pipeline

```
RSS/USPTO Sources → RawSignal → Pipeline (sanitize + compliance + dedup)
  ↓
Evidence Snapshot (SVI ≥ 5 → immutable JSONL evidence log)
  ↓
Wikipedia Enrichment + Full-Text Extraction
  ↓
EntitySanctionDb Extraction (entity → dedup → persist)
  ↓
Scan Agent v1.1 (4-class tags, 3-tier triage: Insight/Watchlist/Memory)
  ↓
LLM Pre-dedup (semantic dedup before clustering)
  ↓
Theme Clustering (≤5 themes, ≥2 articles each)
  ↓
Founder Analysis (What happened / Why it matters / What changed / What to do / What to watch)
  ↓
Causal Chain Extraction (A → B → C → D)
  ↓
Blue Team Verification (load-bearing assumptions, SVI downgrade)
  ↓
DiGraph Cognitive Engine (QE → Belief Engine → Decision Engine)
  ↓
BeliefDb Snapshot (support/challenge/contradiction accumulation)
  ↓
Change Detection (rule or LLM: conflict/reinforce/irrelevant)
  ↓
Trend Layer (14-day category trend: daily_category_stats SQLite table)
  ↓
Dual-Track Emission: HTML (EN/ZH) + Markdown (Astro)
```

## Features

| Feature | Layer | Status |
|---------|-------|--------|
| 29 data sources with Source Scoring (score 1-10 per source) | 0 | ✅ |
| Compliance filter (A-stock codes + stock promotion) | 1 | ✅ |
| SVI Strategic Volatility Index (source_score × recency × signal_strength) | 1 | ✅ |
| Source Scoring (SourceConfig.score + recency factor in SVI) | 1 | ✅ |
| LLM pre-dedup (semantic dedup before clustering) | 1 | ✅ |
| Evidence Snapshot (SVI ≥ 5 → immutable JSONL evidence log) | 1 | ✅ |
| EntitySanctionDb extraction (14 entities: ARM, NVIDIA, OpenAI, etc.) | 1 | ✅ |
| Evidence Snapshot (SVI ≥ 5 → immutable JSONL evidence log) | 1 | ✅ |
| Founder Analysis (What happened / Why it matters / What changed / What to do / What to watch) | 1 | ✅ |
| Causal Chain (A → B → C → D extraction) | 1 | ✅ |
| Blue Team verification (load-bearing assumption challenge, SVI downgrade) | 2 | ✅ |
| DiGraph cognitive engine (QE → Belief Engine → Decision Engine) | 2 | ✅ |
| Change Detection (rule + LLM: conflict/reinforce/irrelevant) | News | ✅ |
| Trend Layer (14-day category trend in HTML) | News | ✅ |
| Source Health monitor (per-source success/failure tracking) | News | ✅ |
| Astro frontend (Sulix Daily layout: Top 3 + Next + Folded) | News | ✅ |
| Research report system (priced tiers: free/premium/enterprise, Stripe-ready) | Research | ✅ |
| Memory Dashboard (BeliefDb: support/challenge/contradiction tracking) | Memory | ✅ |
| LLM Audit (AtomicU64 counters: calls, input tokens, output tokens) | Infra | ✅ |
| Versioned pipeline (uuid_v7 + atomic write + resume) | Infra | ✅ |
| LayeredCache + CircuitBreaker + RetryConfig | Infra | ✅ |
| Substack API integration (Markdown → Draft API) | Biz | ✅ |
| Bilingual EN/ZH (language-specific routing) | All | ✅ |
| Serde deny_unknown_fields (strict config validation) | Sec | ✅ |
| html_escape (37 usages across all render functions) | Sec | ✅ |

## Configuration

`config.toml` key sections:

| Section | Purpose |
|---------|---------|
| `[llm]` | API key, model, endpoint |
| `[[sources]]` | RSS feeds with name, URL, category, layer, score (1-10), public |
| `[prompts]` | Base + domain-specific system prompts |
| `[prompts.vertical_overrides]` | Domain-specific analytical frameworks |
| `[news_layer]` | LLM pre-dedup, Change Detection, RSSHub base URL |
| `[questions]` | Active decision questions for Question Engine |
| `[graveyard]` | Decay Agent settings (retention, compression) |

### Source Layers

| Layer | Name | Frontend Display |
|-------|------|-----------------|
| 1 | Internal intelligence (FT, Economist, Stratechery) | ❌ Hidden (LLM only) |
| 2 | Official sources (Federal Register, SEC, arXiv) | ✅ Full attribution links |
| 3 | Community (HN, GitHub) | ✅ Attribution links |
| 4 | Market (A-stock) | ✅ Attribution links |

## Deployment

### Self-host RSSHub (optional, for Chinese sources)

```bash
docker run -d --name rsshub -p 1200:1200 diygod/rsshub
export RSSHUB_BASE_URL=http://localhost:1200
```

### Frontend

```bash
cd astro-frontend
npm run build
# Output: dist/ → deploy to Cloudflare Pages
```

### Pipeline

```bash
# Daily cron (Linux/macOS)
0 6 * * * cd /path/to/Sulix-Intelligence && cargo run --release >> data/pipeline.log 2>&1

# Daily cron (Windows Task Scheduler)
cargo run --release
```

## License

MIT
