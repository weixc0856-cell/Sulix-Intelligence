<p align="center">
  <a href="README.md">🇬🇧 English</a> · <a href="README.zh-CN.md">🇨🇳 中文</a>
</p>

<p align="center">
  <img src="assets/logo.svg" width="120" alt="Sulix Intelligence" />
</p>

# Sulix Intelligence

> **面向独立开发者与个人创业者的 AI 战略情报助手。**

每日自动生成麦肯锡/BCG/高盛格式的战略情报简报，写入 Obsidian 知识库并通过 Cloudflare Pages 部署为静态 HTML。

## 管线

```
RSS / YouTube → 源适配器 → Pipeline 中间件 → 并发抓取 → Delta 去重
                           (清洗 + HTTP       (feed-rs)
                            重试 + 去重)
                                │
                                ▼
                           Scan Agent
                           （轻量 LLM 初筛）
                                │
                                ▼
                         ┌─── 主题聚类 ───┐
                         │  (LLM 聚合为    │
                         │  ≤5 个战略主题) │
                         └────────────────┘
                                │
                                ▼
                           主题分析
                           (Fact Base 表,
                            信号强度,
                            证据等级)
                                │
                                ▼
                    ┌─── Chronicle Dashboard ───┐
                    │  （追加到 JSON 历史数据库， │
                    │   用于长线主题追踪）        │
                    └────────────────────────────┘
                                │
                                ▼
                        咨询级简报
                    （执行摘要 → 主题分析 →
                     综合判断 → 选项评估 → Kill List）
                                │
              ┌─────────────────┴─────────────────┐
              ▼                                   ▼
      模板引擎渲染                            模板引擎渲染
      （Markdown）                            （HTML）
              │                                   │
              ▼                                   ▼
  DailyBrief/YYYY-MM-DD.md              index.html → Cloudflare
  （Obsidian 知识库）                                   │
                                                     🌐 全球 CDN
                                                     ⚡ 零成本
```

## 技术栈

`Rust` + `feed-rs` + `scraper` + `reqwest` + `tokio` + `rusqlite` + `DeepSeek API` + `Tailwind CSS` + `Cloudflare Pages` + `GitHub Actions`

## 快速开始

```bash
# 1. 克隆并编译
git clone https://github.com/weixc0856-cell/Sulix-Intelligence.git
cd Sulix-Intelligence
cargo build --release

# 2. 配置
cp config.example.toml config.toml
# 编辑 config.toml —— 填写你的 DeepSeek API Key 和 RSS 源

# 3. 运行（输出到默认目录）
cargo run --release

# 或指定自定义输出目录：
VAULT_PATH=/path/to/your/vault cargo run --release

# 输出: DailyBrief/YYYY-MM-DD.md (Markdown)
#       DailyBrief/index.html (Tailwind HTML)
```

## 功能

| 功能 | 状态 |
|------|------|
| RSS/Atom/JSON Feed + YouTube RSS 抓取 | ✅ |
| **HTTP 重试** — 指数退避重试 RSS 抓取失败 | ✅ |
| **HTML 清洗** — 保留 LLM 有用标签，剥离有害标签 | ✅ |
| **Delta 去重** — Jaccard 标题相似度合并 | ✅ |
| **Scan Agent** — 初筛过滤噪音/广告 | ✅ |
| **主题聚类** — LLM 聚合为 ≤5 个战略主题 | ✅ |
| **Fact Base 分析** — 证据|解读|置信度 三栏表 | ✅ |
| **咨询级简报** — 麦肯锡/BCG/高盛格式 | ✅ |
| **选项评估** — 多选项对比/"必须为真"前提检查 | ✅ |
| **Kill List** — 明确"不做什么" | ✅ |
| **Chronicle Dashboard** — JSON 历史数据库，长线主题追踪 | ✅ |
| **模板引擎** — 纯 Rust 占位符替换（零依赖） | ✅ |
| **DataCatalog** — 每步 JSON 审计落盘 | ✅ |
| **中英双语** — 支持英文和中文双输出 | ✅ |
| **经济学人风格品牌** — 红色印章 Logo、SVG Favicon | ✅ |
| **GitHub Actions CI/CD** — 每日自动运行 + Cloudflare 部署 | ✅ |
| **VAULT_PATH 环境变量** — 运行时指定输出目录 | ✅ |
| **HTML 静态内参** — Tailwind CSS，Cloudflare 就绪 | ✅ |
| 支持繁体中文、韩文、日文（通过 template.rs） | 🟡 |
| Wikipedia API 上下文注入 | 🟡 旧版 |
| 关键词预过滤（高吞吐源降噪） | 🟡 旧版 |

## 架构

```
src/
├── main.rs              # 管线编排
├── archive.rs           # Chronicle Dashboard — JSON 历史数据库
├── template.rs          # 模板引擎 — 纯 Rust 占位符替换
├── pipeline.rs          # Pipeline 中间件（清洗、HTML 保留、去重）
├── config.rs            # TOML 配置加载 + DecisionLedger
├── catalog.rs           # DataCatalog — 每步 JSON 审计落盘
├── clusterer.rs         # 主题聚类 + Fact Base 分析
├── db.rs                # SQLite 去重、存储与墓地
├── source/              # 源适配器（RSSHub 风格分发）
│   ├── mod.rs           # 源路由 + RawSignal 结构体
│   └── rss.rs           # RSS 源适配器（含 HTTP 重试）
├── fetcher.rs           # 旧版抓取（迁移至 source/ 中）
├── enricher.rs          # Wikipedia 上下文注入
├── llm.rs               # DeepSeek API 调用（分批+重试）
├── renderer.rs          # 咨询级 Markdown + HTML 简报渲染
└── agent/
    ├── scan.rs          # [Phase A] Scan Agent — 初筛
    ├── editor.rs        # DecisionLens — 文章→决策匹配
    ├── synthesis.rs     # [Phase B] 红军（商业机会分析）
    ├── verification.rs  # [Phase B] 蓝军（风险审计）
    ├── orchestrator.rs  # [Phase B] 仲裁
    ├── calibration.rs   # [Phase C] 认知校准
    └── decay.rs         # [Phase D] 记忆墓地
```

## 输出格式

主题聚类模式下，简报遵循麦肯锡/BCG/高盛结构：

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

### Kill List（明确不做）
- Agent 框架对比研究 — 已商品化，差异化空间小
- 模型能力深度评测 — 决策价值递减

🤖 认知校准
```

## 配置说明

`config.toml` 配置段：

- `[llm]` — API Key、模型、接口地址
- `[[sources]]` — RSS 源（名称、URL、分类、类型、关键词、排除关键词）
- `[prompts]` — 系统提示词
- `[prompts.vertical_overrides]` — 垂直领域专属框架
- `[decisions]` — DecisionLedger（活跃决策追踪）
- `[scan_agent]` — Scan Agent 配置
- `[graveyard]` — Decay Agent 配置

源适配器支持配置：
- `keywords` — 正向关键词白名单（文章需匹配至少一个）
- `exclude_keywords` — 反向关键词黑名单（匹配即丢弃）
- `date_range` — "d7" = 最近 7 天，"h24" = 最近 24 小时等

### 信息源层级

| 层级 | 名称 | 说明 |
|------|------|------|
| 1 | 信号源 | 官方博客、YouTube 技术频道 |
| 2 | 精选源 | 已有人替你过滤 |
| 3 | 社区源 | HN、Reddit |
| 4 | 市场源 | GitHub Trending、融资数据 |

## 部署

### GitHub Actions（推荐）

内置的 `.github/workflows/daily.yml` 通过 cron 定时每日运行管线。
推送到 GitHub 后配置 Secrets（DEEPSEEK_API_KEY），Cloudflare Pages 自动部署生成的 `index.html`。

### 手动运行

```bash
cargo run --release
# 输出: DailyBrief/index.html → CF Pages
# 零服务器成本、全球 CDN、免 ICP 备案
```

## 许可

MIT
