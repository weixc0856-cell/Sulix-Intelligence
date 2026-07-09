<p align="center">
  <a href="README.md">🇬🇧 English</a> · <a href="README.zh-CN.md">🇨🇳 中文</a>
</p>

# Sulix Intelligence

> **全自动认知引擎 — 个人创业者的战略决策操作系统。**

Sulix Intelligence 将原始信号转化为结构化**战略记忆**—— Signal → Assessment → Decision → Outcome。回答的不是「发生了什么」，而是「这件事是否改变我未来 6 个月的决策」。

```
原始信号 (RSS/USPTO/Reddit)
    ↓
管线 → Scan Agent → 主题聚类
    ↓
认知引擎 (Memory + Hermes + Decision)
    ↓
ArtifactSet (Signals / Assessments / Decisions / Outcomes)
    ↓
Schema 验证门 (拒绝不完整对象)
    ↓
本地存储 + R2 (不可变资产) + 前端同步
    ↓
MDX 视图 (从 JSON artifact 派生)
```

## 架构

```
                    sulix-engine (Rust)
                           |
                    ArtifactSet JSON
                  ┌─────────┼─────────┐
                  ↓         ↓         ↓
                 R2        D1       Frontend
              (资产)     (索引)     (Astro UI)
                           |
                    Cloudflare Worker
                     JSON API Layer
                           |
                    Astro UI Shell
                  (Bloomberg Terminal)
```

## 三仓储架构

| 仓库 | 职责 | 技术栈 |
|------|------|--------|
| **sulix-engine** ← 本仓库 | 数据采集、分析、战略记忆 | Rust + DeepSeek API |
| [sulix-web](https://github.com/weixc0856-cell/Intel-Web) | UI 壳、导航、UX | Astro + Tailwind |
| **sulix-docs** | 产品决策、架构、ADR | Obsidian Markdown |

## 三层产品

| 产品 | 目标 | 价格 |
|------|------|------|
| **News Layer** | 获客 | $0 |
| **Research Layer** | 收入 | $99-$4999 |
| **Memory Layer** | 护城河 | 不对外 |

## 快速开始

```bash
# 1. 编译
git clone https://github.com/weixc0856-cell/Sulix-Intelligence.git
cd Sulix-Intelligence
cp config.example.toml config.toml
cargo build --release

# 2. 运行（需要 DEEPSEEK_API_KEY）
export DEEPSEEK_API_KEY="sk-..."
cargo run --release

# 3. 前端预览
cd ../sulix-web
cp -r ../Sulix-Intelligence/output/* src/content/
npm install && npm run dev
```

## 管线

```
数据源采集 (RSS / USPTO / Reddit)
    ↓ 管线：清洗 → 合规 → 去重
    ↓ 证据快照 (不可变 JSONL, SVI ≥ 5)
    ↓ Scan Agent v1.1 (三层分流: Insight / Watchlist / Signal Memory)
    ↓ LLM 预去重 → 主题聚类 (≤5 主题)
    ↓ 主题分析 + ASI/Confidence 评分
    ↓ 蓝军验证 (承重假设挑战)
    ↓ Editor Agent (个人影响分析)
    ↓ MemoryEngine (Thesis / Evidence / Outcome / Reflection)
    ↓ Hermes (变更检测 / 趋势 / 冲突)
    ↓ Decision Intelligence (Thesis → Decision 映射)
    ↓ Meta Layer (自动 Outcome 检测 + Reflection 生成)
    ↓ 验证门 (schema::validator)
    ↓ Artifact Publisher → 本地 + R2 + 前端同步
    ↓ Event Log flush (data/events/{date}.jsonl)
```

## 代码结构

```
src/
├── domain/           — 9 领域模型 (+ Localized, + Day 3 Belief proposal)
├── engine/           — 认知引擎 (analysis/memory/premium/belief/decision)
├── publishing/       — 5 阶段发布协调器 → 返回 ArtifactSet
├── artifact/         — Manifest/Report/Builder (纯函数)
├── delivery/         — 验证门 → 本地 → R2 → 前端同步 + Event flush
├── translation/      — LLM 文件级翻译 (Phase 1 过渡桥梁)
├── schema/           — Schema 验证 (schemars derive + Validate trait)
├── storage/          — R2 上传客户端 (S3 兼容), 损坏恢复辅助
├── renderer/         — MDX/Markdown/HTML 渲染 (MDX 从 JSON 派生)
├── hermes/           — 变更检测 + 趋势 + 冲突
├── clusterer/        — 主题聚类 + LLM 预去重 + 合成
├── agent/            — Scan Agent + Editor Agent + Calibration + Decay
├── source/           — 数据源适配器 (RSS/USPTO/Reddit)
├── event_log/        — ObjectEvent 审计追踪 (追加式 JSONL)
├── bin/outcome.rs    — Outcome 追踪 CLI (record/list/audit)
├── main.rs           — 管线编排 (~500 行)
└── lib.rs            — 模块声明
```

## Schema 验证门

每个 artifact 在存储前通过验证。被拒对象写入 `data/rejected/{date}/`，触发非零退出码。

| 检查项 | Phase 0 | Phase 1 |
|--------|---------|---------|
| 必填字段非空 | ✅ | ✅ |
| Confidence 在 [0,1] | ✅ | ✅ |
| Evidence 数组非空 | ⚠️ 警告 | ❌ 拒绝 |
| Decision 类型合法 | ✅ | ✅ |

## 事件审计

所有对象生命周期事件记录在 `data/events/{date}.jsonl`：

```json
{"schema_version":1,"event_type":"decision_created","object_id":"DEC-0001","summary":{"confidence":0.72}}
{"schema_version":1,"event_type":"outcome_recorded","object_id":"OUT-001","summary":{"verdict":"PartiallyConfirmed"}}
{"schema_version":1,"event_type":"publish_completed","summary":{"passed":3,"rejected":0,"r2_status":"not_configured"}}
```

事件只含摘要字段（不含全量快照），完整对象历史在 R2 中。

## 翻译（本地化资产）

Phase 1 过渡层：LLM 驱动的文件级翻译，将 MDX 输出翻译为 `zh-cn` 和 `zh-tw` 变体。

```
Engine 输出 (en) → Translation Agent → zh-cn/*.md + zh-tw/*.md
```

`src/translation/` 模块处理完整性检查、模型覆盖和追踪元数据。每个语言版本的 frontmatter 嵌入 `is_translated`、`machine_translated` 等跟踪字段以实现下游审计。

## Outcome 追踪 CLI

```
cargo run --bin outcome
```

独立 CLI 工具，用于记录和审查决策结果：
- `outcome record <id> <verdict>` — 记录新结果
- `outcome list` — 列出最近结果
- `outcome audit <id>` — 完整审计线索及置信度历史

## 配置

| 配置段 | 用途 |
|--------|------|
| `[llm]` | API key、模型、端点 |
| `[[sources]]` | 数据源 (名称、URL、分类、层级、评分) |
| `[prompts]` | 各分析阶段的系统提示词 |
| `[output]` | 输出路径 (vault_path, mdx_dir, frontend_public_dir) |
| `[storage]` | data_dir 持久化状态 |
| `[r2]` | Cloudflare R2 配置 (bucket, endpoint, public_url) |

## 部署

### CI 管线 (GitHub Actions)

```yaml
# .github/workflows/cron_brief.yml
# 每日：cargo run --release → R2 → sulix-web 构建 → CF Pages
```

所需 Secrets：
- `DEEPSEEK_API_KEY` — LLM 提供商
- `R2_ACCESS_KEY_ID` / `R2_SECRET_ACCESS_KEY` / `R2_ENDPOINT` — R2 存储
- `CLOUDFLARE_API_TOKEN` / `CLOUDFLARE_ACCOUNT_ID` — Pages 部署

## 许可证

MIT
