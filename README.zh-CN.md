<p align="center">
  <a href="README.md">🇬🇧 English</a> · <a href="README.zh-CN.md">🇨🇳 中文</a>
</p>

<p align="center">
  <img src="assets/logo.svg" width="120" alt="Sulix Intelligence" />
</p>

# Sulix Intelligence

> **全自动数字 AI 智库 — 个人创业者的认知操作系统。**

Sulix Intelligence 是三个独立产品共享同一套底层 Rust 管线的三层架构：

```
                 Source Layer（21+ 数据源）
                      │
               Signal Layer（SVI + 合规 + 聚类）
                      │
          ┌───────────┼────────────┐
          │           │            │
    News Layer  Research Layer  Memory Layer
    （免费）     （付费报告）    （私有认知资产）
```

- **News Layer** → Bloomberg Terminal 风格看板。每日信号聚合。免费。
- **Research Layer** → 多 Agent 深度研报。$99-$4999。付费。
- **Memory Layer** → 信念追踪、矛盾检测、决策历史。私有。

**回答的问题不是"发生了什么"，而是"这件事是否改变我未来 6 个月的决策"。**

## 架构

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
                              (公开)            Dashboard
```

### 技术栈

| 层 | 技术 |
|-------|-------|
| 后端 | Rust + feed-rs + scraper + reqwest + tokio + rusqlite |
| LLM | DeepSeek / OpenAI API（自带 Key） |
| 前端 | Astro + TypeScript + JetBrains Mono + Inter |
| 缓存 | LayeredCache（内存 HashMap + TTL）+ CircuitBreaker |
| 认证 | Substack（邮件订阅）+ Stripe/LemonSqueezy（报告） |
| 部署 | Cloudflare Pages + GitHub Actions |
| 成本 | ~$0/月 基础设施 + LLM API（~$3/月） |

### 三个产品

| 产品 | 目的 | 形式 | 价格 |
|--------|---------|--------|-------|
| **News Layer** | 获客 | Terminal Dashboard · 每日邮件 | $0 |
| **Research Layer** | 收入 | 多 Agent 研报 · PDF 下载 | $99-$4999 |
| **Memory Layer** | 护城河 | 信念追踪 · 决策历史 | 私有 |

## 快速开始

```bash
# 1. 克隆并编译
git clone https://github.com/weixc0856-cell/Sulix-Intelligence.git
cd Sulix-Intelligence
cargo build --release

# 2. 配置
cp config.example.toml config.toml
# 填写 DeepSeek API Key 和数据源

# 3. 运行
cargo run --release

# 输出：
#   DailyBrief/en/YYYY-MM/index.html  → News Layer（本地看板）
#   content/posts/                    → Astro Markdown
#   data/belief_db.json              → Memory Layer

# 4. 构建前端
cd astro-frontend
npm install && npm run build

# 5. 启动前端开发服务器
npm run dev        # → http://localhost:4321
```

## 管线

```
RSS/USPTO 源 → RawSignal → Pipeline（清洗 + 合规 + 去重）
  ↓
证据快照（SVI ≥ 5 → 不可变 JSONL 证据日志）
  ↓
Wikipedia 注入 + 正文提取
  ↓
EntitySanctionDb 实体提取（去重 → 持久化）
  ↓
Scan Agent v1.1（4 类标签，Insight/Watchlist/Memory 三层分流）
  ↓
LLM 预去重（语义去重，聚类前合并同一事件的文章）
  ↓
主题聚类（最多 5 个主题，每个 ≥2 篇文章）
  ↓
创始人分析（What happened / Why it matters / What changed / What to do / What to watch）
  ↓
因果链提取（A → B → C → D）
  ↓
蓝军验证（承重假设检测，SVI 降级）
  ↓
DiGraph 认知引擎（QE → Belief Engine → Decision Engine）
  ↓
BeliefDb 快照（支持/挑战/矛盾累积）
  ↓
变更检测（规则或 LLM：冲突/强化/无关）
  ↓
趋势层（14 天类别趋势统计：daily_category_stats SQLite 表）
  ↓
双轨产出：HTML（中英文）+ Markdown（Astro 前端）
```

## 功能

| 功能 | 产品层 | 状态 |
|---------|-------|--------|
| 29 数据源 + Source Scoring（score 1-10） | 0 | ✅ |
| 合规熔断（A 股代码 + 荐股词过滤） | 1 | ✅ |
| SVI 战略异动指数（source_score × recency × signal_strength） | 1 | ✅ |
| Source Scoring（SourceConfig.score + recency 因子） | 1 | ✅ |
| LLM 预去重（聚类前语义去重） | 1 | ✅ |
| 证据快照（SVI ≥ 5 → JSONL 证据日志） | 1 | ✅ |
| EntitySanctionDb 实体提取（ARM/NVIDIA/OpenAI 等） | 1 | ✅ |
| 创始人分析（What happened / What changed / What to do） | 1 | ✅ |
| 因果链提取（A → B → C → D） | 1 | ✅ |
| 蓝军验证（承重假设挑战，SVI 降级） | 2 | ✅ |
| DiGraph 认知引擎（QE → Belief Engine → Decision Engine） | 2 | ✅ |
| 变更检测（规则 + LLM：冲突/强化/无关） | News | ✅ |
| 趋势层（14 天类别趋势） | News | ✅ |
| 源健康监控 | News | ✅ |
| Astro 前端（Sulix Daily 三级布局） | News | ✅ |
| 研究报告系统（free/premium/enterprise 三级） | Research | ✅ |
| Memory 看板（BeliefDb：支持/挑战/矛盾） | Memory | ✅ |
| LLM 审计（AtomicU64 计数器） | Infra | ✅ |
| Serde deny_unknown_fields（严格配置验证） | Sec | ✅ |
| html_escape（37 处覆盖） | Sec | ✅ |
| 研究报告系统（定价分层，Stripe 就绪） | Research | ✅ |
| 记忆仪表盘（BeliefDb + 矛盾追踪） | Memory | ✅ |
| 版本化管线（uuid_v7 + 原子写入 + 断点恢复） | 基建 | ✅ |
| LayeredCache + CircuitBreaker + RetryConfig | 基建 | ✅ |
| RSSHub URL 重写（环境变量 RSSHUB_BASE_URL） | 基建 | ✅ |
| Substack API 集成 | 商业 | ✅ |
| Flash Mode 紧急加更（SVI ≥ 9） | News | ✅ |
| 特殊专题人工注入（.flash/*.json） | News | ✅ |
| 中英双语 | 全部 | ✅ |
| 哲学注入（三易/第一性原理/道家/飞轮/金字塔） | 2 | ✅ |
| 社会科学范式（科斯/贝克/康波周期） | 2 | ✅ |

## 配置

`config.toml` 主要配置段：

| 配置段 | 用途 |
|---------|---------|
| `[llm]` | API Key、模型、接口地址 |
| `[[sources]]` | RSS 源（名称、URL、分类、层级、公开性） |
| `[prompts]` | 基础 + 领域专用系统提示词 |
| `[prompts.vertical_overrides]` | 垂直领域分析框架 |
| `[news_layer]` | LLM 预去重、Change Detection、RSSHub 地址 |
| `[questions]` | Question Engine 的活跃决策问题 |
| `[graveyard]` | Decay Agent 设置（保留期限、压缩） |

### 数据源层级

| 层级 | 名称 | 前端显示 |
|-------|------|-----------------|
| 1 | 内参学习源（FT、Economist、Stratechery） | ❌ 隐藏（仅 LLM 熔炼） |
| 2 | 官方权威源（Federal Register、SEC、arXiv） | ✅ 完整溯源链接 |
| 3 | 社区源（HN、GitHub） | ✅ 溯源链接 |
| 4 | 市场源（A 股） | ✅ 溯源链接 |

## 部署

### 自建 RSSHub（可选，用于修复国内源）

```bash
docker run -d --name rsshub -p 1200:1200 diygod/rsshub
export RSSHUB_BASE_URL=http://localhost:1200
```

### 前端

```bash
cd astro-frontend
npm run build
# 输出：dist/ → 部署到 Cloudflare Pages
```

### 管线定时运行

```bash
# Linux/macOS 定时任务
0 6 * * * cd /path/to/Sulix-Intelligence && cargo run --release >> data/pipeline.log 2>&1

# Windows 任务计划程序
cargo run --release
```

## 许可

MIT
