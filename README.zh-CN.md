<p align="center">
  <a href="README.md">🇬🇧 English</a> · <a href="README.zh-CN.md">🇨🇳 中文</a>
</p>

<p align="center">
  <img src="assets/logo.svg" width="120" alt="Sulix Intelligence" />
</p>

# Sulix Intelligence

> **全自动数字 AI 智库 — 个人创业者的认知操作系统。**

Sulix Intelligence 是一个**认知引擎**，将原始信号处理为结构化知识资产。

```
原始信号 (RSS/USPTO/Reddit)
    ↓
管线 (清洗 + 合规 + 去重)
    ↓
分析引擎 (SVI + ASI + Confidence + 主题聚类)
    ↓
蓝军验证 + Editor Agent
    ↓
记忆引擎 (Thesis + Evidence + Outcome + Reflection)
    ↓
MDX 知识资产 → sulix-web (前端) → Cloudflare Pages
```

**回答的问题：** 不是「发生了什么」——而是「这件事是否改变我未来 6 个月的决策」。

## 三仓储架构

| 仓库 | 职责 | 技术栈 |
|------|------|--------|
| **sulix-engine** ← 本仓库 | 数据采集、分析、记忆、内容生成 | Rust + feed-rs + DeepSeek API |
| [sulix-web](https://github.com/weixc0856-cell/Intel-Web) | 渲染、导航、UX | Astro + Tailwind + design.css |
| **sulix-docs** | 产品决策、架构、ADR、研究 | Obsidian Markdown |

跨仓库职责变更需记录 ADR。

## 三层产品

| 产品 | 目标 | 格式 | 价格 |
|------|------|------|------|
| **News Layer** | 获客 | 每日 MDX 信号 | $0 |
| **Research Layer** | 收入 | 多 Agent 研报 · MDX | $99-$4999 |
| **Memory Layer** | 护城河 | Thesis 追踪 · MDX | 不对外 |

## 快速开始

```bash
# 1. 克隆并编译
git clone https://github.com/weixc0856-cell/Sulix-Intelligence.git
cd Sulix-Intelligence
cargo build --release

# 2. 配置
cp config.example.toml config.toml
# 在 [llm] 中设置 DeepSeek API key
# 在 [output] 中设置 mdx_dir = "output"

# 3. 运行
cargo run --release

# 输出：
#   output/daily/YYYY-MM-DD-slug.mdx    → 每日信号
#   output/thesis/YYYY-MM-DD-slug.mdx   → 判断追踪
#   output/research/YYYY-MM-DD-slug.mdx → Premium 研报
#   output/reflection/YYYY-MM-DD-slug.mdx → 复盘反思

# 4. 前端预览（sulix-web）
git clone https://github.com/weixc0856-cell/Intel-Web.git
cd Intel-Web
cp -r ../Sulix-Intelligence/output/* src/content/
npm install && npm run dev  # → http://localhost:4321
```

## 管线

```
RSS/USPTO 数据源 → RawSignal → 管线 (清洗 + 合规 + 去重)
  ↓
证据快照 (SVI ≥ 5 → 不可变 JSONL)
  ↓
Wikipedia 注入 + 正文提取
  ↓
实体提取 (EntitySanctionDb)
  ↓
Scan Agent v1.1 (4 类标签，3 层分流)
  ↓
LLM 预去重 → 主题聚类 (≤5 主题)
  ↓
主题分析 (BLUF / Impact / Geopolitical / Supply Chain / Causal Chains)
  ↓
ASI + Confidence 评分
  ↓
蓝军验证 (承重假设挑战)
  ↓
DiGraph 认知引擎 (QE → Belief Engine → Decision Engine)
  ↓
Editor Agent (个人影响分析)
  ↓
变更检测 + 趋势层
  ↓
MemoryEngine (Thesis + Evidence + Outcome + Reflection)
  ↓
MDX 输出: daily/  thesis/  research/  reflection/
```

## MDX 知识格式

```mdx
---
title: AI Agent Infrastructure Consolidation
date: 2026-06-24
svi: 8.7
asi: 7.5
confidence: 0.81
type: daily
---

## BLUF

一句话核心结论。
```

该格式：
- **Git 友好** — diff、审查、历史
- **Astro 原生** — `getCollection("daily")` 直接消费
- **人类可读** — 任意文本编辑器可编辑

## 功能状态

| 功能 | 状态 |
|------|------|
| 29 个数据源 + 源评分 | ✅ |
| SVI 战略异动指数 | ✅ |
| ASI + Confidence 评分 | ✅ |
| Scan Agent 3 层分流 | ✅ |
| Editor Agent (个人影响) | ✅ |
| 蓝军验证 | ✅ |
| DiGraph 认知引擎 | ✅ |
| MemoryEngine (Thesis + Outcome + Reflection) | ✅ |
| Belief Engine Phase B (WayneOPC) | ✅ |
| Meta Layer (自动结果检测) | ✅ |
| MDX 知识输出 (6 集合) | ✅ |
| Twitter/X 推文管线 | ✅ |
| Reddit 数据源 | ✅ |
| 变更检测 (规则 + LLM) | ✅ |
| Event Log (审计日志) | ✅ |
| Chronicle (历史数据库) | ✅ |
| 双语 EN/ZH | ✅ |
| Substack API 集成 | ✅ |

### 代码结构 (65+ 文件)

```
src/
├── domain/        — 7 领域模型
├── engine/        — 核心引擎
├── hermes/        — 变更检测 + 趋势
├── renderer/      — MDX 输出 + HTML
├── clusterer/     — 主题聚类
├── agent/         — Scan/Editor/Calibration/Decay
├── source/        — 数据源适配器
├── twitter.rs     — 推文管线
├── publishing.rs  — Publishing Agent
├── event_log.rs   — 事件日志
└── main.rs        — 管线编排
```

## 部署

### 管线 (cron)

```bash
# Linux/macOS: 每日 06:00
0 6 * * * cd /path/to/Sulix-Intelligence && cargo run --release

# Windows
cargo run --release
```

### 前端 (sulix-web)

前端为独立仓库：[sulix-web](https://github.com/weixc0856-cell/Intel-Web)

```bash
git clone https://github.com/weixc0856-cell/Intel-Web.git
cd Intel-Web
cp -r ../Sulix-Intelligence/output/* src/content/
npm install && npm run build
```

## 许可证

MIT
