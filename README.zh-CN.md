<p align="center">
  <a href="README.md">🇬🇧 English</a> · <a href="README.zh-CN.md">🇨🇳 中文</a>
</p>

# Sulix Intelligence

> **面向独立开发者与个人创业者的 AI 战略情报助手。**

每日自动生成麦肯锡/BCG 格式的战略情报简报，写入 Obsidian 知识库或部署为静态 HTML 页面。

## 管线

```
RSS / YouTube / Wikipedia → 并发抓取 → Delta 去重 → SQLite 去重
                                    │
                                    ▼
                            [Phase A] Scan Agent
                            （轻量 LLM 初筛）
                                    │
                                    ▼
                            主题聚类
                            （LLM 聚合为 ≤5 个主题）
                                    │
                                    ▼
                            主题分析
                            （Fact Base 证据表、信号强度、证据等级）
                                    │
                                    ▼
                        咨询级简报
                        （执行摘要 → 主题分析 → 综合判断 → 选项评估 → Kill List）
                                    │
              ┌──────────────────────┴──────────────────────┐
              ▼                                             ▼
    Markdown → Obsidian 知识库           HTML → Cloudflare Pages
    (DailyBrief/YYYY-MM-DD.md)          (DailyBrief/index.html)
```

## 技术栈

`Rust` + `feed-rs` + `scraper` + `reqwest` + `tokio` + `rusqlite` + `DeepSeek API` + `Wikipedia API` + `Tailwind CSS` + `Cloudflare Pages`

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
# 输出: DailyBrief/YYYY-MM-DD.md (Markdown)
#       DailyBrief/index.html (Tailwind HTML)
```

## 功能

| 功能 | 状态 |
|------|------|
| RSS/Atom/JSON Feed + YouTube RSS 抓取 | ✅ |
| 全文提取（scraper） | ✅ |
| **Delta 去重** — Jaccard 标题相似度合并 | ✅ |
| Wikipedia 上下文注入（中文→英文回退） | ✅ |
| **Scan Agent** — 初筛过滤噪音/广告 | ✅ |
| **主题聚类** — LLM 将文章聚合为 ≤5 个战略主题 | ✅ |
| **Fact Base 分析** — 证据|解读|置信度 三栏表 | ✅ |
| **咨询级简报** — 麦肯锡/BCG/高盛格式 | ✅ |
| **选项评估** — 多选项对比 + "必须为真"前提检查 | ✅ |
| **Kill List** — 明确"不做什么" | ✅ |
| **DataCatalog** — 每步 JSON 审计落盘 | ✅ |
| **DecisionLedger** — 追踪活跃决策与证据状态 | ✅ |
| **Calibration Agent** — 认知偏差探测 | ✅ |
| **关键词预过滤** — 正则白名单处理高吞吐源 | ✅ |
| **HTML 静态内参** — Tailwind CSS，Cloudflare 就绪 | ✅ |

## 架构

```
src/
├── main.rs              # 管线编排
├── config.rs            # TOML 配置加载 + DecisionLedger
├── catalog.rs           # DataCatalog — 每步 JSON 审计落盘
├── clusterer.rs         # 主题聚类 + Fact Base 分析
├── db.rs                # SQLite 去重、存储与墓地
├── fetcher.rs           # RSS/YouTube 抓取 + 全文提取 + Delta 去重
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
- `[[sources]]` — RSS 源（名称、URL、分类、层级）
- `[prompts]` — 系统提示词
- `[prompts.vertical_overrides]` — 垂直领域专属框架
- `[decisions]` — DecisionLedger（活跃决策追踪）
- `[scan_agent]` — Scan Agent 配置
- `[graveyard]` — Decay Agent 配置

### 信息源层级

| 层级 | 名称 | 说明 |
|------|------|------|
| 1 | 信号源 | 官方博客、Wikipedia API、YouTube 技术频道 |
| 2 | 精选源 | 已有人替你过滤 |
| 3 | 社区源 | HN、Reddit |
| 4 | 市场源 | GitHub Trending、融资数据 |

## 许可

MIT
