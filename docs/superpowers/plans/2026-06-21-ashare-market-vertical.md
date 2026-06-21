# A 股 Market Vertical Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add A股 (A-share) market analysis vertical to Sulix Intelligence with 5 new RSS sources, a 6-dimension Red-Blue analysis framework, and a keyword pre-filter to manage high-throughput sources.

**Architecture:** Config-only changes (RSS sources + prompt override) plus a keyword pre-filter in `fetcher.rs` that uses regex to drop 60-70% of noise from 财联社 telegram before it reaches the LLM pipeline. No new files needed.

**Tech Stack:** Rust, regex (new crate), RSSHub (external service), existing feed-rs pipeline

**Files modified:**
- `config.toml` — add 5 RSS sources + A股 override
- `config.example.toml` — sync example
- `src/fetcher.rs` — add keyword pre-filter for high-throughput A股 sources
- `Cargo.toml` — add `regex` dependency

---

### Task 1: Add `regex` dependency to Cargo.toml

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add regex crate**

```toml
# 正文提取（P0）
scraper = "0.22"

# 稳定哈希（替代 std DefaultHasher，跨 Rust 版本稳定）
sha2 = "0.10"

# A股关键词预过滤（高频源降噪）
regex = "1.11"
```

Add `regex = "1.11"` after `sha2` in the `[dependencies]` section.

- [ ] **Step 2: Verify cargo check**

Run: `cargo check 2>&1`
Expected: `Finished dev profile` with no errors

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add regex dependency for A股 keyword pre-filter"
```

---

### Task 2: Add keyword pre-filter to fetcher.rs

**Files:**
- Modify: `src/fetcher.rs`

Add a keyword filter function and integrate it into `fetch_single_source` so that high-frequency A股 sources (财联社电报) get noise-reduced before entering the pipeline.

- [ ] **Step 1: Add the filter function before `fetch_single_source`**

After line `use crate::config::SourceConfig;` (line 10), add:

```rust
use regex::Regex;
```

After the `fetch_all_sources` function (after line 60, before `fetch_single_source`), add:

```rust
/// A 股关键词白名单 — 只保留包含核心增量信息的条目
/// 财联社等源单日可产出数百条，先过滤再进 LLM 管线可省 60-70% token
fn ashare_keyword_filter(articles: &mut Vec<Article>) {
    let pattern = r"(?x)
        大盘|指数|沪指|两市|万亿|成交额|成交额|分位|
        板块|轮动|概念|半导体|芯片|AI|算力|光伏|锂电|汽车|智驾|医药|券商|地产|
        主力|净流出|净流入|主力资金|融资|融券|异动|吸筹|出货|
        政策|证监会|央行|国常会|监管|产业基金|补贴|降准|降息|
        财报|预增|净利润|年报|季报|预亏|暴雷|
        北向|外资|南向|港股|恒生|
        涨停|跌停|连板|炸板|封板|打板|
        龙虎榜|游资|机构
    ";
    let re = match Regex::new(pattern) {
        Ok(r) => r,
        Err(_) => return, // 正则失效时保底放行
    };

    articles.retain(|a| {
        let title_match = re.is_match(&a.title);
        let summary_match = a.summary.as_deref().map(|s| re.is_match(s)).unwrap_or(false);
        title_match || summary_match
    });
}
```

- [ ] **Step 2: Integrate filter into `fetch_single_source`**

Replace the end of `fetch_single_source` to apply the filter for A股 sources. Find:

```rust
    log::info!("✅ [{}] → {} 篇文章", source.name, articles.len());
    Ok(articles)
```

Replace with:

```rust
    let mut articles = articles;
    // 高吞吐 A 股源（财联社等）执行关键词预过滤，砍掉 60-70% 噪音
    if source.category == "A股" {
        let before = articles.len();
        ashare_keyword_filter(&mut articles);
        let filtered = before - articles.len();
        if filtered > 0 {
            log::info!(
                "🔍 [{}] 关键词过滤: {} → {} 篇 (移除 {} 篇噪音)",
                source.name,
                before,
                articles.len(),
                filtered
            );
        }
    }

    log::info!("✅ [{}] → {} 篇文章", source.name, articles.len());
    Ok(articles)
```

- [ ] **Step 3: Verify cargo check**

Run: `cargo check 2>&1`
Expected: `Finished dev profile` with no errors

- [ ] **Step 4: Commit**

```bash
git add src/fetcher.rs
git commit -m "feat: add A股 keyword pre-filter to reduce telegraph noise by 60-70%"
```

---

### Task 3: Add A股 RSS sources to config.toml

**Files:**
- Modify: `config.toml`

- [ ] **Step 1: Add 5 new sources**

Insert before the `# ===== Phase A: Scan Agent =====` line (currently line 144 in config.toml but may vary). Add:

```toml
# ===== A 股市场垂直 =====
# 通过 RSSHub 桥接国内金融信息源，每日吞吐量较高
# 财联社电报需配合 fetcher.rs 的关键词预过滤降噪
[[sources]]
name = "东方财富-要闻"
type = "rss"
url = "https://rsshub.app/eastmoney/news"
category = "A股"
layer = 2
enabled = true

[[sources]]
name = "财联社-电报"
type = "rss"
url = "https://rsshub.app/cls/telegraph"
category = "A股"
layer = 3
enabled = true

[[sources]]
name = "同花顺-聚焦"
type = "rss"
url = "https://rsshub.app/10jqka/focus"
category = "A股"
layer = 2
enabled = true

[[sources]]
name = "证券时报"
type = "rss"
url = "https://rsshub.app/stcn/news"
category = "A股"
layer = 2
enabled = true

[[sources]]
name = "财联社-政策频道"
type = "rss"
url = "https://rsshub.app/cls/channels/1075"
category = "政策"
layer = 1
enabled = true
```

- [ ] **Step 2: Verify TOML parses**

Run: `cargo check 2>&1`
Expected: `Finished dev profile` with no errors

- [ ] **Step 3: Commit**

```bash
git add config.toml
git commit -m "feat: add 5 A股 RSS sources via RSSHub for market vertical"
```

---

### Task 4: Add A股 vertical_override prompt to config.toml

**Files:**
- Modify: `config.toml`

- [ ] **Step 1: Add A股 override to `[prompts.vertical_overrides]`**

Find the existing overrides block (starts after `[prompts.vertical_overrides]`). It currently has `"AI"`, `"创业"`, `"芯片"`, `"政策"`. Add `"A股"` at the end:

```toml
"政策" = """
...
"""  (existing closing)

"A股" = """
你是一个身经百战的 A 股顶级对冲基金宏观策略分析师。
请对以下打包的盘中/每日信息流进行【六维交叉比对】，必须严格执行红蓝对抗：

## 六维框架

1. 资金面 — 主力资金布局方向、北向资金趋势、融资融券变化
2. 技术面 — 换手率结构、成交量、板块轮动位置
3. 政策面 — 产业政策红利方向、监管放松信号
4. 基本面 — 低估价值洼地、景气度拐点、估值分位
5. 情绪面 — 市场过冷=买入窗口、散户追高风险
6. 背离检测（核心） — 跨维度交叉比对：价格 vs 资金背离、情绪 vs 资金背离、政策 vs 量价背离

## 红蓝对抗指令

- 🔴 红军视角：寻找资金在主力布局、政策红利共振、真正景气度拐点的【真布局】机会。
- 🔵 蓝军视角：死盯着大盘放量滞涨、高位换手率异常、有利好无量、小作文无端煽动的【假掩护】出货陷阱。

## 背离检测规则

- 情绪面 vs 资金面：电报一片看好、涨停家数新高，但主力资金放量滞涨 → 输出【高危背离：散户情绪过热，主力资金假掩护】
- 政策面 vs 技术面：政策大利好，但板块高开低走，成交量无法持续放大 → 输出【利好出尽，市场流动性不足，轮动进入末期】

## 信号质量分级（A股含金量漏斗）

- L1 杂音：个股盘中波动、常规聚焦、无主线小作文 → 忽略
- L2 异动：板块短线换手率突破、单日主力净流入 → 观察，需交叉验证
- L3 共振：政策发文 + 主力资金连续 3 日净流入 + 技术形态共振 → 研究
- L4 主线趋势：行业景气度拐点 + 央行/产业政策强力落地 → 可行动
- L5 系统窗口：市场冰点期 + 估值历史洼地 + 大周期信号 → 立即行动

## 产出要求

- 【盘中/每日焦点综述】用一句话总结当前主线
- 【六维交叉对抗透视】逐项分析各维度红蓝博弈
- 【A股含金量漏斗评级】给出明确的 L1-L5 标签
- 【轮动所处阶段判断】主线早期 / 轮动中期 / 退潮末期
"""
```

- [ ] **Step 2: Verify TOML parses and tests pass**

Run: `cargo test 2>&1`
Expected: 39 passed, 0 failed

- [ ] **Step 3: Commit**

```bash
git add config.toml
git commit -m "feat: add A股 vertical_override with 6-dimension analysis framework"
```

---

### Task 5: Sync config.example.toml

**Files:**
- Modify: `config.example.toml`

- [ ] **Step 1: Add A股 sources to example**

Insert after the commented-out `Stratechery` block and before `[prompts]`:

```toml
# [[sources]]
# name = "东方财富-要闻"
# type = "rss"
# url = "https://rsshub.app/eastmoney/news"
# category = "A股"
# layer = 2
# enabled = false

# [[sources]]
# name = "财联社-电报"
# type = "rss"
# url = "https://rsshub.app/cls/telegraph"
# category = "A股"
# layer = 3
# enabled = false
# # 警告：此源单日数百条，必须搭配 fetcher.rs 的关键词预过滤使用

# [[sources]]
# name = "财联社-政策频道"
# type = "rss"
# url = "https://rsshub.app/cls/channels/1075"
# category = "政策"
# layer = 1
# enabled = false
```

- [ ] **Step 2: Add A股 override to example**

Find the existing `[prompts.vertical_overrides]` section. After the `"政策"` override closing `"""`, add:

```toml
# "A股" = """
# A 股全维度分析框架，详见 docs/superpowers/specs/2026-06-21-ashare-market-vertical-design.md
# 核心：六维交叉比对（资金面/技术面/政策面/基本面/情绪面/背离检测）
# 红蓝对抗：红军找真布局，蓝军抓假掩护
# 信号分级：A股含金量漏斗 L1-L5
# """
```

- [ ] **Step 3: Verify**

Run: `cargo test 2>&1`
Expected: 39 passed, 0 failed (config.example.toml is loaded by `test_config_from_example`, must parse)

- [ ] **Step 4: Commit**

```bash
git add config.example.toml
git commit -m "docs: add A股 vertical example sources and override to config.example.toml"
```

---

### Task 6: Run full verification

**Files:** (none — verification only)

- [ ] **Step 1: Full verification suite**

```bash
cargo clippy -- -D warnings 2>&1 && cargo test 2>&1 && cargo fmt --check 2>&1
```

Expected: All pass (clippy 0 errors, 39 tests, fmt 0 diff)

- [ ] **Step 2: (Optional) Run pipeline**

```bash
cargo run --release 2>&1 | tail -30
```

Verify that:
- A股 sources are fetched without errors
- Keyword filter logs show filtered count (e.g. "关键词过滤: 120 → 35 篇")
- DailyBrief generates with A股 section

- [ ] **Step 3: Final commit (if cargo fmt or fixes needed)**

```bash
git add -A && git commit -m "chore: final cleanup after A股 vertical implementation"
```
