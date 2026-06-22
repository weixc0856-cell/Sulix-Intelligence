<p align="center">
  <a href="README.md">🇬🇧 English</a> · <a href="README.zh-CN.md">🇨🇳 中文</a>
</p>

# Sulix Intelligence

> **Personal AI strategic intelligence assistant for indie founders and solo developers.**

Daily automatically generated intelligence briefings in McKinsey/BCG consulting format. Written to your Obsidian vault or deployed as static HTML.

## Pipeline

```
RSS / YouTube / Wikipedia → Concurrent fetch → Delta dedup → SQLite dedup
                                    │
                                    ▼
                            [Phase A] Scan Agent
                            (lightweight LLM filter)
                                    │
                                    ▼
                            Theme Clustering
                            (LLM groups into ≤5 themes)
                                    │
                                    ▼
                            Theme Analysis
                            (Fact Base table, signal strength, evidence level)
                                    │
                                    ▼
                        Consulting-Grade Briefing
                        (Exec Summary → Theme Analysis → Synthesis → Options → Kill List)
                                    │
                                    ▼
              ┌──────────────────────┴──────────────────────┐
              ▼                                             ▼
    Markdown → Obsidian vault             HTML → Cloudflare Pages
    (DailyBrief/YYYY-MM-DD.md)            (DailyBrief/index.html)
```

## Tech Stack

`Rust` + `feed-rs` + `scraper` + `reqwest` + `tokio` + `rusqlite` + `DeepSeek API` + `Wikipedia API` + `Tailwind CSS` + `Cloudflare Pages`

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
# Output: DailyBrief/YYYY-MM-DD.md (Markdown)
#         DailyBrief/index.html (Tailwind HTML)
```

## Features

| Feature | Status |
|---------|--------|
| RSS/Atom/JSON Feed + YouTube RSS fetching | ✅ |
| Full-text extraction (scraper) | ✅ |
| **Delta dedup** — Jaccard bigram similarity merge | ✅ |
| Wikipedia context injection (zh → en fallback) | ✅ |
| **Scan Agent** — pre-filter noise/ads | ✅ |
| **Theme Clustering** — LLM groups articles into ≤5 strategic themes | ✅ |
| **Fact Base Analysis** — Evidence | Interpretation | Confidence table per theme | ✅ |
| **Consulting-grade Report** — McKinsey/BCG/Goldman format | ✅ |
| **Option Evaluation** — multi-choice with "must be true" preconditions | ✅ |
| **Kill List** — explicit "what we are NOT doing" | ✅ |
| **DataCatalog** — JSON audit trail per pipeline step | ✅ |
| **DecisionLedger** — track active decisions with evidence state | ✅ |
| **Calibration Agent** — cognitive bias probing | ✅ |
| **Keyword pre-filter** — regex whitelist for high-throughput sources | ✅ |
| **HTML static page** — Tailwind CSS, Cloudflare-ready | ✅ |
| Markdown daily briefing generation | ✅ |

## Architecture

```
src/
├── main.rs              # Pipeline orchestration
├── pipeline.rs          # Middleware chain (sanitize, compliance, dedup)
├── config.rs            # TOML config loader + DecisionLedger
├── catalog.rs           # DataCatalog — JSON audit trail per step
├── clusterer.rs         # Theme clustering + Fact Base analysis
├── db.rs                # SQLite dedup, storage & graveyard
├── source/              # Source adapters (RSSHub-style dispatch)
│   ├── mod.rs           # Source routing + RawSignal struct
│   └── rss.rs           # RSS feed adapter
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
| GLM-5.2成本降幅超预期 | 差旅门槛进一步降低 | L3 |
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
- `[[sources]]` — RSS feeds with name, URL, category, layer
- `[prompts]` — system prompts
- `[prompts.vertical_overrides]` — domain-specific frameworks
- `[decisions]` — DecisionLedger (active decisions being tracked)
- `[scan_agent]` — Scan Agent settings
- `[graveyard]` — Decay Agent settings

### Source Layers

| Layer | Name | Description |
|-------|------|-------------|
| 1 | Signal Source | Official blogs, Wikipedia API, YouTube tech channels |
| 2 | Curated Source | Pre-filtered by humans |
| 3 | Community Source | HN, Reddit |
| 4 | Market Source | GitHub Trending, funding data |

## License

MIT
