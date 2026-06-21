<p align="center">
  <a href="README.md">🇬🇧 English</a> · <a href="README.zh-CN.md">🇨🇳 中文</a>
</p>

# Sulix Intelligence

> **面向独立开发者与个人创业者的 AI 战略情报助手。**

每日自动生成情报简报，直接写入你的 Obsidian 知识库或任意 Markdown 目录。

## 管线

```
RSS 源 → 并发抓取 → SQLite 去重 → 正文提取 → 按分类分组
          (feed-rs)    (rusqlite)   (scraper)
                           │
               ┌───────────┴───────────┐
               ▼                       ▼
       [Phase A] Scan Agent     (过滤噪音/广告)
               │
               ▼
       [Phase B] 红蓝对抗
          ├─ 🔴 红军（乐观叙事）
          ├─ 🔵 蓝军（怀疑反驳）
          └─ ⚖️  仲裁（合并意见）
               │
               ▼
       [Phase C] Calibration Agent（认知偏差校准）
               │
               ▼
       Markdown 日报 → DailyBrief/（含辩论痕迹）
               │
               ▼
       [Phase D] Decay Agent（记忆墓地）
          ├─ 埋葬过期旧文章
          └─ 唤醒重复出现的主题
```

## 技术栈

`Rust` + `feed-rs` + `scraper` + `reqwest` + `tokio` + `rusqlite` + `DeepSeek API` + `Markdown` + `Cron`

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
```

## 功能

| 功能 | 状态 |
|------|------|
| RSS/Atom/JSON Feed 抓取 | ✅ |
| 全文提取（scraper） | ✅ |
| SQLite 去重与存储 | ✅ |
| LLM 分析（DeepSeek）分批调用 | ✅ |
| 指数退避重试 | ✅ |
| **Scan Agent** — 分析前过滤噪音/广告 | ✅ |
| **红蓝对抗** — 乐观叙事 + 怀疑反驳 + 仲裁 | ✅ |
| **Calibration Agent** — 认知偏差探测（每日一问） | ✅ |
| **Decay Agent** — 记忆墓地 + 唤醒信号 | ✅ |
| Markdown 日报生成 | ✅ |
| Cron 定时调度 | ✅ |

## 架构

```
src/
├── main.rs               # 管线编排（Phase A→B→C→D）
├── config.rs             # TOML 配置加载
├── db.rs                 # SQLite 去重、存储与墓地查询
├── fetcher.rs            # 并发 RSS 抓取 + 正文提取
├── llm.rs                # DeepSeek API 调用（分批+重试）
├── renderer.rs           # Markdown 日报渲染
└── agent/
    ├── mod.rs            # 模块声明
    ├── scan.rs           # [Phase A] Scan Agent — 快速初筛
    ├── synthesis.rs      # [Phase B] 🔴 红军 — 乐观叙事
    ├── verification.rs   # [Phase B] 🔵 蓝军 — 怀疑反驳
    ├── orchestrator.rs   # [Phase B] ⚖️  仲裁 — 合并红蓝
    ├── calibration.rs    # [Phase C] 🤖 认知校准 — 偏差提问
    └── decay.rs          # [Phase D] 🪦 Decay Agent — 记忆墓地
```

核心驱动力是 **Lens Library** — 编码为系统提示词的领域判断框架。真正的差异化不在于代码，而在于你注入到每个垂直领域分析 prompt 中的认知框架。

## Agent 管线说明

文章抓取并分组后，管线依次执行 4 个 agent 阶段：

**Phase A — Scan Agent（扫描员）。** 每批文章用一次轻量 LLM 调用，给每篇文章打重要性分（1-10）。低于阈值（默认 ≤3）的跳过，视为噪音/PR/广告。在深入分析前先过滤，节省 token 开销。

**Phase B — 红蓝对抗。** 两个独立的 LLM 回合，角色完全对立：
- 🔴 **红军（Synthesis）**：乐观叙事构建者。跨源关联线索，识别趋势，发现机会。
- 🔵 **蓝军（Verification）**：极端怀疑者。用证据等级（L1-L5）和"AI 神话拆解六问"挑战每一条判断。
- ⚖️ **仲裁（Orchestrator）**：纯逻辑（不调 LLM）。合并红蓝输出，标记 L4/L5 风险，确认 L1/L2 共识。

**Phase C — Calibration Agent（认知校准）。** 每天在简报底部追加一个尖锐问题。探测当天分析中的认知盲点和矛盾。不是为了提供答案，而是为了让你思考。

**Phase D — Decay Agent（记忆墓地）。** 日报写完后的后台维护：埋葬超过保留期（默认 90 天）的文章，可选用 LLM 压缩后存入，并检查今天的新文章是否有匹配已埋葬主题的 — 如有则触发"唤醒信号"追加到当日简报。

## 判断框架

每篇文章从 5 个维度评估：

| 维度 | 评分 |
|------|------|
| 重要性 | 1-10 |
| 相关性 | 高 / 中 / 低 |
| 时间跨度 | 短期 / 中期 / 长期 |
| 可行动性 | 立即行动 / 研究 / 观察 / 忽略 |
| 信心等级 | 高 / 中 / 低 |

## 配置说明

`config.toml` 是整个系统的大脑。关键配置段：

- `[llm]` — API Key、模型、接口地址
- `[[sources]]` — RSS 源，每条含名称、URL、分类、层级
- `[prompts]` — 每个垂直领域的系统提示词（这是你的核心竞争力）
- `[scan_agent]` — Phase A：开关、重要性阈值
- `[agent]` — Phase B：开关 Synthesis 和 Verification
- `[graveyard]` — Phase D：保留天数、压缩、埋葬阈值
- `[storage]` — SQLite 数据库目录
- `[output]` — 日报输出路径
- `[dedup]` — 去重窗口和标题相似度阈值

### 信息源层级

源按四层模型组织：

| 层级 | 名称 | 说明 |
|------|------|------|
| 1 | 信号源 | 官方博客，最准确但最难读 |
| 2 | 精选源 | 已有人替你过滤，信号质量最高 |
| 3 | 社区源 | 热点先于主流媒体在这里爆发 |
| 4 | 市场源 | 招聘、融资、开源趋势 |

## 许可

MIT
