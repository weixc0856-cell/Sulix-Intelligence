<p align="center">
  <a href="README.md">🇬🇧 English</a> · <a href="README.zh-CN.md">🇨🇳 中文</a>
</p>

<p align="center">
  <img src="assets/logo.svg" width="120" alt="Sulix Intelligence" />
</p>

# Sulix Intelligence

> **Personal AI strategic intelligence assistant for indie founders and solo developers.**

Daily automatically generated intelligence briefings in McKinsey/BCG/Goldman consulting format. Written to your Obsidian vault and deployed as static HTML via Cloudflare Pages.

## Pipeline

```
RSS / YouTube → Source Adapters → Pipeline Middleware → Concurrent fetch → Delta dedup
                                   (sanitize + HTTP        (feed-rs)
                                    retry + dedup)
                                         │
                                         ▼
                                    Scan Agent
                                    (lightweight LLM filter)
                                         │
                                         ▼
                              ┌── Theme Clustering ──┐
                              │  (LLM groups into     │
                              │   ≤5 strategic themes)│
                              └──────────────────────┘
                                         │
                                         ▼
                              Theme Analysis
                              (Fact Base table,
                               signal strength,
                               evidence level)
                                         │
                                         ▼
                    ┌─── Chronicle Dashboard ───┐
                    │  (append to JSON history   │
                    │   DB for long-term tracking)│
                    └────────────────────────────┘
                                         │
                                         ▼
                         Consulting-Grade Briefing
                    (Exec Summary → Theme Analysis →
                     Synthesis → Options → Kill List)
                                         │
                    ┌────────────────────┴────────────────────┐
                    ▼                                         ▼
          Template Engine                              Template Engine
          (Markdown output)                             (HTML output)
                    │                                         │
                    ▼                                         ▼
          DailyBrief/YYYY-MM-DD.md                  index.html → Cloudflare
          (Obsidian vault)                                        │
                                                              🌐 Global CDN
                                                              ⚡ Zero-cost
```

## Tech Stack

`Rust` + `feed-rs` + `scraper` + `reqwest` + `tokio` + `rusqlite` + `DeepSeek API` + `Tailwind CSS` + `Cloudflare Pages` + `GitHub Actions`

## Quick Start

```bash
# 1. Clone and build
git clone https://github.com/weixc0856-cell/Sulix-Intelligence.git
cd Sulix-Intelligence
cargo build --release

# 2. Configure
cp config.example.toml config.toml
# Edit config.toml — set your DeepSeek API key and RSS sources

# 3. Run (output to default vault)
cargo run --release

# Or run with custom vault path:
VAULT_PATH=/path/to/your/vault cargo run --release

# Output: DailyBrief/YYYY-MM-DD.md (Markdown for vault)
#         DailyBrief/index.html (Tailwind HTML for Cloudflare)
```

## Features

| Feature | Status |
|---------|--------|
| RSS/Atom/JSON Feed + YouTube RSS fetching | ✅ |
| **HTTP retry** — exponential backoff for RSS fetch failures | ✅ |
| **HTML sanitization** — preserves LLM-useful tags, strips harmful | ✅ |
| **Delta dedup** — Jaccard bigram similarity merge | ✅ |
| **Scan Agent** — pre-filter noise/ads | ✅ |
| **Theme Clustering** — LLM groups articles into ≤5 strategic themes | ✅ |
| **Fact Base Analysis** — Evidence | Interpretation | Confidence table per theme | ✅ |
| **Consulting-grade Report** — McKinsey/BCG/Goldman format | ✅ |
| **Option Evaluation** — multi-choice with "must be true" preconditions | ✅ |
| **Kill List** — explicit "what we are NOT doing" | ✅ |
| **Chronicle Dashboard** — JSON history DB for long-term topic tracking | ✅ |
| **Template Engine** — pure Rust placeholder substitution (zero deps) | ✅ |
| **DataCatalog** — JSON audit trail per pipeline step | ✅ |
| **Bilingual EN/ZH** — both English and Chinese output | ✅ |
| **Economist-style branding** — red seal logo, SVG favicon | ✅ |
| **GitHub Actions CI/CD** — automated daily runs + Cloudflare deploy | ✅ |
| **VAULT_PATH env var** — override output directory at runtime | ✅ |
| **HTML static page** — Tailwind CSS, Cloudflare-ready | ✅ |
| Support for traditional Chinese, Korean, Japanese (via template.rs) | 🟡 |
| Wikipedia API context injection | 🟡 Legacy |
| Keyword pre-filter for high-throughput sources | 🟡 Legacy |

## Architecture

```
src/
├── main.rs              # Pipeline orchestration
├── archive.rs           # Chronicle Dashboard — JSON history DB for long-term tracking
├── template.rs          # Template engine — pure Rust placeholder substitution
├── pipeline.rs          # Middleware chain (sanitize, HTML preservation, dedup)
├── config.rs            # TOML config loader + DecisionLedger
├── catalog.rs           # DataCatalog — JSON audit trail per step
├── clusterer.rs         # Theme clustering + Fact Base analysis
├── db.rs                # SQLite dedup, storage & graveyard
├── source/              # Source adapters (RSSHub-style dispatch)
│   ├── mod.rs           # Source routing + RawSignal struct
│   └── rss.rs           # RSS feed adapter with HTTP retry
├── fetcher.rs           # Legacy fetch (being migrated to source/)
├── enricher.rs          # Wikipedia API context injection
├── llm.rs               # DeepSeek API calling with batching & retry
├── renderer.rs          # Consulting-grade Markdown + HTML briefing
└── agent/
    ├── scan.rs          # [Phase A] Scan Agent
    ├── editor.rs        # DecisionLens — article→decision matching
    ├── synthesis.rs     # [Phase B] Red team (business opportunity)
    ├── verification.rs  # [Phase B] Blue team (risk audit)
    ├── orchestrator.rs  # [Phase B] Arbitration
    ├── calibration.rs   # [Phase C] Cognitive bias probing
    └── decay.rs         # [Phase D] Memory graveyard
```

## Output Format

When theme clustering is active, the briefing follows McKinsey/BCG/Goldman structure:

```
# Sulix Intelligence — 2026-06-22

## 执行摘要
1. **模型商品化加速** — 开源能力接近闭锁（3 条证据）
2. **Agent可靠性成为焦点** — 工程化标准确立（2 条证据）

## 主题: 模型商品化

| 证据 | 解读 | 置信度 |
|------|------|--------|
| GLM-5.2成本降幅超预期 | 创业门槛进一步降低 | L3 |
| OpenAI跟进行业定价 | 头部竞争加剧 | L2 |

信号强度: 7/10 — 行业机制级

## 综合判断
**结论**: 模型差异化缩小，应用层窗口打开。

## 战略建议
| 选项 | 必须为真的前提 | 风险 | 信心 |
|------|--------------|------|------|
| 继续应用层深挖 | 价格战不压缩利润空间 | L3 |

### Kill List (明确不做)
- Agent 框架对比研究 — 已商品化，差异化空间小
- 模型能力深度评测 — 决策价值递减

🤖 认知校准
```

## Configuration

`config.toml` sections:

- `[llm]` — API key, model, endpoint
- `[[sources]]` — RSS feeds with name, URL, category, type, layer, keywords, exclude_keywords
- `[prompts]` — system prompts
- `[prompts.vertical_overrides]` — domain-specific frameworks
- `[decisions]` — DecisionLedger (active decisions being tracked)
- `[scan_agent]` — Scan Agent settings
- `[graveyard]` — Decay Agent settings

Source adapter config supports:
- `keywords` — positive keyword whitelist (article must match at least one)
- `exclude_keywords` — negative keyword blacklist (article dropped on match)
- `date_range` — "d7" = last 7 days, "h24" = last 24 hours, etc.

### Source Layers

| Layer | Name | Description |
|-------|------|-------------|
| 1 | Signal Source | Official blogs, YouTube tech channels |
| 2 | Curated Source | Pre-filtered by humans |
| 3 | Community Source | HN, Reddit |
| 4 | Market Source | GitHub Trending, funding data |

## Deployment

### GitHub Actions (recommended)

The included `.github/workflows/daily.yml` runs the pipeline daily via cron.
Push to GitHub, configure secrets (DEEPSEEK_API_KEY), and Cloudflare Pages
auto-deploys the resulting `index.html`.

### Manual

```bash
cargo run --release
# Output: DailyBrief/index.html → CF Pages
# Zero server cost, global CDN, no ICP备案 needed
```

## License

MIT
