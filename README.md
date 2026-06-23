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
RSS Sources → RawSignal → Pipeline (sanitize + compliance + dedup)
  ↓
Scan Agent v1.1 (4-class tags, 3-tier triage: Insight/Watchlist/Memory)
  ↓
LLM Pre-dedup (semantic dedup before clustering)
  ↓
Theme Clustering (≤5 themes, ≥2 articles each)
  ↓
Theme Analysis (BLUF + Geopolitical Fact + Supply Chain Impact)
  ↓
Blue Team Verification (load-bearing assumptions, SVI downgrade)
  ↓
3-Agent Council (Diplomat → Architect → Quant)
  ↓
Dual-Track Emission: HTML (Obsidian) + Markdown (Astro)
```

## Features

| Feature | Layer | Status |
|---------|-------|--------|
| 21+ data sources (Federal Register / SEC / arXiv / FT / Economist / HN / etc.) | 0 | ✅ |
| Compliance filter (A-stock codes + stock promotion) | 1 | ✅ |
| SVI Strategic Volatility Index (5-dimension scoring) | 1 | ✅ |
| LLM pre-dedup (semantic dedup before clustering) | 1 | ✅ |
| 3-Agent Council (Diplomat + Architect + Quant) | 2 | ✅ |
| Blue Team verification (load-bearing assumption challenge) | 2 | ✅ |
| TerminationCondition combinators (.and()/.or()) | 2 | ✅ |
| DiGraph orchestration engine (GraphFlow-style) | 2 | ✅ |
| Question Engine (signal-to-question matching) | 3-5 | ✅ |
| Belief Engine (contradiction_score formula) | 3-5 | ✅ |
| Decision Engine (4-tier: NoChange/CourseCorrect/Urgent/StrategicPivot) | 3-5 | ✅ |
| EntitySanctionDb (dual ID + inferred/declared isolation) | 3-5 | ✅ |
| Terminal Dashboard (Bloomberg Terminal style) | News | ✅ |
| Change Detection (LLM semantic conflict detection) | News | ✅ |
| Source Health monitor | News | ✅ |
| Astro frontend (Content Collections v6, Zod schema) | News | ✅ |
| Research report system (priced tiers, Stripe-ready) | Research | ✅ |
| Memory Dashboard (BeliefDb + contradiction tracking) | Memory | ✅ |
| Versioned pipeline (uuid_v7 + atomic write + resume) | Infra | ✅ |
| LayeredCache + CircuitBreaker + RetryConfig | Infra | ✅ |
| RSSHub URL rewrite (env var RSSHUB_BASE_URL) | Infra | ✅ |
| Substack API integration (Markdown → Draft API) | Biz | ✅ |
| Flash Mode (SVI ≥ 9 → red banner + alert) | News | ✅ |
| SpecialTopic injection (.flash/*.json files) | News | ✅ |
| Bilingual EN/ZH (language-specific routing) | All | ✅ |
| Philosophical prompt injection (Three Easies / First-Principles / Daoist dialectics) | 2 | ✅ |
| Social science paradigms (Coase / Beck / K-Waves) | 2 | ✅ |

## Configuration

`config.toml` key sections:

| Section | Purpose |
|---------|---------|
| `[llm]` | API key, model, endpoint |
| `[[sources]]` | RSS feeds with name, URL, category, layer, public |
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
