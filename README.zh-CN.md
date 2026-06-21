<p align="center">
  <a href="README.md">🇬🇧 English</a> · <a href="README.zh-CN.md">🇨🇳 中文</a>
</p>

# Sulix Intelligence

> **面向独立开发者与个人创业者的 AI 战略情报助手。**

每日自动生成情报简报，可写入 Obsidian 知识库，或部署为静态 HTML 页面托管到 Cloudflare Pages。

## 管线

```
RSS / YouTube / Wikipedia → 并发抓取 → Delta 去重（标题相似度合并）
       (feed-rs)                        (Jaccard 二元组 0.75 阈值）
                                    │
                        ┌───────────┴───────────┐
                        ▼                       ▼
                Wikipedia 上下文注入         关键词预过滤
                （中文→英文回退）           （高吞吐源降噪）
                                    │
                                    ▼
                            SQLite 去重
                            （URL hash）
                                    │
                                    ▼
                        ┌─── [Phase A] Scan Agent ───┐
                        │  （轻量 LLM 初筛）          │
                        └─────────────────────────────┘
                                    │
                                    ▼
                        ┌─── [Phase B] 红蓝对抗 ─────┐
                        │  🔴 红军（机会侦察）        │
                        │  🔵 蓝军（风险审计）        │
                        │  ⚖️  仲裁（逐条裁决）       │
                        └────────────────────────────┘
                                    │
                        [Phase C] Calibration Agent
                        （认知偏差探测）
                                    │
              ┌─────────────────────┴─────────────────────┐
              ▼                                           ▼
    Markdown → Obsidian 知识库       HTML → Cloudflare Pages
    (DailyBrief/YYYY-MM-DD.md)       (DailyBrief/index.html)
                                    │
                                    ▼
                        [Phase D] Decay Agent（记忆墓地）
```

## 技术栈

`Rust` + `feed-rs` + `scraper` + `reqwest` + `tokio` + `rusqlite` + `DeepSeek API` + `Wikipedia API` + `HTML/Tailwind` + `Cloudflare Pages`

## 快速开始

```bash
# 1. 克隆并编译
git clone https://github.com/weixc0856-cell/Sulix-Intelligence.git
cd Sulix-Intelligence
cargo build --release

# 2. 配置
cp config.example.toml config.toml
# 编辑 config.toml —— 填写你的 DeepSeek API Key 和 RSS 源

# 3. 运行
cargo run --release
# 输出: DailyBrief/YYYY-MM-DD.md（Markdown 日报）
#       DailyBrief/index.html（Tailwind HTML 静态内参）
```

## 功能

| 功能 | 状态 |
|------|------|
| RSS/Atom/JSON Feed + YouTube RSS 抓取 | ✅ |
| 全文提取（scraper） | ✅ |
| **Delta 去重** — Jaccard 标题相似度合并多源相同新闻 | ✅ |
| SQLite 去重与存储 | ✅ |
| **Wikipedia 上下文注入** — 自动获取中/英文技术词摘要 | ✅ |
| **关键词预过滤** — 正则白名单处理高吞吐源（财联社等） | ✅ |
| LLM 分析（DeepSeek）分批调用 + 指数退避重试 | ✅ |
| **Scan Agent** — 分析前过滤噪音/广告 | ✅ |
| **红蓝对抗** — 机会侦察 + 风险审计 + 逐条仲裁 | ✅ |
| **战略等级（S/A/B/C）** — 范式转移 / 季度影响 / 关注 / 噪音 | ✅ |
| **去 AI 味麦肯锡文风** — 禁用废话词、动词驱动 | ✅ |
| **Calibration Agent** — 认知偏差探测（每日一问） | ✅ |
| **Decay Agent** — 记忆墓地 + 唤醒信号 | ✅ |
| **HTML 静态内参** — Tailwind CSS，Cloudflare 就绪 | ✅ |
| Markdown 日报生成 | ✅ |

## 架构

```
src/
├── main.rs               # 管线编排（Phase A→B→C→D）
├── config.rs             # TOML 配置加载
├── db.rs                 # SQLite 去重、存储与墓地查询
├── fetcher.rs            # 并发抓取 + 正文提取 + 关键词过滤 + Delta 去重
├── enricher.rs           # Wikipedia 上下文注入（中文→英文回退）
├── llm.rs                # DeepSeek API 调用（分批+重试）
├── renderer.rs           # Markdown + Tailwind HTML 日报渲染
└── agent/
    ├── mod.rs            # 模块声明
    ├── scan.rs           # [Phase A] Scan Agent — 快速初筛
    ├── synthesis.rs      # [Phase B] 🔴 红军 — 机会侦察
    ├── verification.rs   # [Phase B] 🔵 蓝军 — 风险审计
    ├── orchestrator.rs   # [Phase B] ⚖️  仲裁 — 逐条裁决
    ├── calibration.rs    # [Phase C] 🤖 认知校准 — 偏差提问
    └── decay.rs          # [Phase D] 🪦 Decay Agent — 记忆墓地
```

## 输出格式

红蓝模式下，每篇文章渲染为决策卡片：

```
📌 今日核心信号

**标题** — 重要性:8/10 | 战略:A | 信心:L4
💬 一句话大白话摘要（≤40 字）

🔴 红军: 商业机会——谁受益、为什么是现在（≤60 字）
🔵 蓝军: 执行风险——隐藏成本、证据等级（≤60 字）
⚖️ 仲裁: 逐条仲裁结论
🎯 我的判断: 针对创始人的具体建议

---

<details>📦 其他信号（N 条）...</details>

🤖 认知校准
```

## 判断框架

每篇文章从创业者视角评估：

| 维度 | 评分 |
|------|------|
| 战略等级 | S / A / B / C（范式转移 / 季度影响 / 关注 / 噪音） |
| 重要性 | 1-10 |
| 证据等级 | L1（数学证明）- L5（营销炒作） |
| 可行动性 | 立即行动 / 研究 / 观察 / 忽略 |

## 写作风格（去 AI 味）

输出遵循麦肯锡/高盛专业服务标准：
- **动词驱动**：硬数据 + 强动词，零形容词
- **结论先行**：永远先说结论
- **红蓝各 ≤ 60 字**：禁止废话
- **禁用词**：惊人、炸裂、不可否认、双刃剑、值得注意的是、总而言之、时代的浪潮

## 配置说明

`config.toml` 是整个系统的大脑。关键配置段：

- `[llm]` — API Key、模型、接口地址
- `[[sources]]` — RSS 源，每条含名称、URL、分类、层级
- `[prompts]` — 基础 + 垂直领域系统提示词（核心竞争力）
- `[prompts.vertical_overrides]` — 领域专属框架：AI、技术主线、创业、A股、芯片、政策
- `[scan_agent]` — Phase A：开关、重要性阈值
- `[agent]` — Phase B：开关 Synthesis 和 Verification
- `[graveyard]` — Phase D：保留天数、压缩、埋葬阈值

### 信息源层级

| 层级 | 名称 | 说明 |
|------|------|------|
| 1 | 信号源 | 官方博客、Wikipedia API、YouTube 技术频道 |
| 2 | 精选源 | 已有人替你过滤，信号质量最高 |
| 3 | 社区源 | HN、Reddit — 先于媒体爆发 |
| 4 | 市场源 | GitHub Trending、融资数据 |

## 部署

生成 HTML 静态内参后部署到 Cloudflare Pages：

```bash
cargo run --release
# 输出: DailyBrief/index.html → CF Pages
# 零服务器成本、全球 CDN、免 ICP 备案
```

## 许可

MIT
