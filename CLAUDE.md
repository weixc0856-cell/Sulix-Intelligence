# Sulix Intelligence — Claude 开发指南

## Core Chain

```
Observation → Signal → Thesis → Investigation → Decision → Outcome → Reflection → Memory
```

Sulix 是一个 Decision Intelligence System。核心链路已全部闭环。

## 项目结构

```
src/
├── domain/           ← 8 个领域模型 (Theme/Thesis/Evidence/Investigation/Observation/Action/Outcome/Reflection)
├── engine/           ← 核心引擎
│   ├── analysis/     ← 主题分析、ASI 评分、SVI 计算
│   ├── belief/       ← BeliefEngineV2 (WayneOPC)
│   ├── decision/     ← Thesis → Decision 映射 + DecisionStability
│   ├── investigation/← Thesis → 结构化问题生成 (LLM)
│   ├── memory/       ← MemoryEngine v2 (证据去重/置信度追踪/生命周期/Outcome)
│   └── premium/      ← Premium 研报生成
├── hermes/           ← 变更检测 + 趋势/矛盾/Thesis 发现
├── renderer/         ← MDX 输出 (主要) + Markdown (次要)
│   ├── mdx.rs        ← MDX 知识资产渲染器 (主要输出格式)
│   ├── publisher.rs  ← Publisher trait + MdxPublisher
│   ├── helpers.rs    ← html_escape + yaml_escape
│   ├── markdown.rs   ← Substack Markdown
│   └── premium.rs    ← Premium 研报 HTML 渲染
├── agent/            ← Scan Agent + Editor Agent + Calibration + Decay
├── clusterer/        ← 聚类 + 共享类型
├── source/           ← 源适配器 (RSS/USPTO/Reddit)
├── question_engine.rs ← 问答引擎 (关键词/LLM 匹配，与 orchestrator DiGraph 集成)
├── publishing.rs     ← Publishing Agent 编排 (含 Investigation + Decision Intelligence)
├── event_log.rs      ← PipelineEvent 审计日志
├── orchestrator.rs   ← DiGraph 认知编排引擎
├── llm.rs            ← LLM 调用 + JSON 解析
├── config.rs         ← 配置加载
├── db.rs             ← SQLite 数据库
└── main.rs           ← 管线编排 (use sulix_intel::*, 无 mod 声明)
```

## 常用命令

```bash
cargo check             # 编译检查 (单次编译, 无 dual compilation)
cargo clippy            # lint 检查（保持 0 警告）
cargo test              # 运行测试 (129 tests)
cargo fmt               # 格式化
cargo run --release     # 运行完整管线
```

## 架构原则

- **4-Agent 管线**: init → agent_signal → agent_research → agent_publish
- **MDX 是 view model, 不是 canonical state**: Rust engine 是唯一 truth source
- **前端仓库**: [Intel-Web](https://github.com/weixc0856-cell/Intel-Web) (Astro + Content Collections, 纯展示层)
- **engine/ 目录**: 核心领域逻辑，独立于管线编排
- **认知引擎**: DiGraph (orchestrator) + Decision Intelligence (engine/decision) + MemoryEngine (engine/memory)
- **No dual compilation**: main.rs 使用 `use sulix_intel::*` 而非 `mod` 声明

## 输出结构

```
output/
├── daily/       ← 每日信号 MDX (每个 theme 一个文件, 含 Personal Impact)
├── thesis/      ← 判断追踪 MDX (含 Decision/Stability/Outcome 元数据)
├── assessment/  ← ASM-XXXX 规范命名判断文件 (与 thesis 内容一致)
├── decision/    ← DEC-XXXX 独立决策记录
├── research/    ← Premium 研报 MDX
├── investigation/ ← INV-XXXX 调研记录
└── reflection/  ← 复盘反思 MDX (自动生成, Outcome 驱动)
```

## 决策智能

```
MemoryEngine.theses[]
  ↓
map_theses_to_decisions()      ← 确定性映射规则 (非 LLM)
  ↓
ThesisDecision { decision_type, horizon, rationale, stability }
  ↓
MdxPublisher → thesis MDX frontmatter → intel-web DecisionBadge
```

- **6 种决策类型**: Build / Invest / Monitor / Learn / Ignore / Exit
- **4 种时间尺度**: Immediate / 30d / 90d / 180d
- **3 种稳定性**: Volatile (无 outcome) / Stable (majority confirmed) / Final (invalidated)
- **Outcome → Reflection 自动生成**: 每次记录 outcome 自动触发

## Skill routing

- `review` — 代码审查
- `qa` — QA 测试
- `ship` — 部署
- `investigate` — 调试问题
