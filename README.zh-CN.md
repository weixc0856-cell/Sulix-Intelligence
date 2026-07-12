<p align="center">
  <a href="README.md">🇬🇧 English</a> · <a href="README.zh-CN.md">🇨🇳 中文</a>
</p>

# Sulix Intelligence

> **全自动认知引擎 — 个人创业者的战略决策操作系统。**

Sulix Intelligence 将原始信号转化为结构化**战略记忆**—— Signal → Assessment → Decision → Outcome。回答的不是「发生了什么」，而是「这件事是否改变我未来 6 个月的决策」。

```
原始信号 (RSS/USPTO/Reddit)
    ↓
SignalClassificationStep — Fast Path (规则) | Slow Path (LLM)
    ↓
ThesisGenerationStep — 匹配已有 | LLM 生成新判断
    ↓
DecisionMappingStep — RuleEngine + 可选 LlmJudge
    ↓
PipelineStats + MDX 输出
```

## 架构

```
crates/                    cargo run -p sulix-cli
├── sulix-contract/       ← 层间契约类型 (Observation/Signal/Thesis/Decision)
├── sulix-config/         ← TOML 配置加载
├── sulix-llm/            ← LLM Provider (client/retry/api/parser/dispatch/audit)
├── sulix-intelligence/   ← 认知管线 (PipelineStep trait + 3 步骤)
├── sulix-observation/    ← 数据源适配器 (RSS/Reddit/USPTO)
└── sulix-cli/            ← 命令行入口 (sulix-cli + intel-pipeline)
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
cargo build --release -p sulix-cli

# 2. 运行（需要 DEEPSEEK_API_KEY）
export DEEPSEEK_API_KEY="sk-..."
cargo run -p sulix-cli --release

# 3. 管线独立调试（使用 mock 输入）
cargo run --bin intel-pipeline -- --input tests/fixtures/observation.json
```

## 管线

```
crates/sulix-observation/     — 数据源采集 (RSS / USPTO / Reddit)
    ↓  RawSignal → Article → Observation
    ↓
crates/sulix-cli/src/agent/  — Signal Agent: 抓取 → SQLite 去重 → 丰富
    ↓  Vec<Observation>
    ↓
crates/sulix-intelligence/
    │
    ├── SignalClassificationStep  — Fast Path (规则) | Slow Path (LLM 批处理)
    │   observation → Signal { importance, domain, category, why }
    │
    ├── ThesisGenerationStep     — 匹配已有 | LLM 生成新 Thesis
    │   signal → Thesis { claim, confidence, evidence, status }
    │
    └── DecisionMappingStep      — RuleEngine + 可选 LlmJudge
        thesis → Decision { action, horizon, confidence, reasoning }
    ↓
    后处理: calibration (LLM 扎心问题) + summary (规则摘要)
    ↓
    MDX 输出: thesis/{slug}.md + decision/{slug}.md
```

## 代码结构

```
crates/
├── sulix-contract/     — 层间契约类型 (7 模型: Observation/Signal/Thesis/Decision/Theme/Belief/Reflection)
├── sulix-config/       — TOML 配置加载 (LlmConfig, SourceConfig, OutputConfig)
├── sulix-llm/          — LLM Provider (client/retry/api/parser/dispatch/audit — 7 子模块)
├── sulix-intelligence/ — 认知管线 (PipelineStep<Observation, Signal, Thesis, Decision>)
│   ├── step.rs          — PipelineStep trait + PipelineStats
│   ├── signal_classification.rs — 双路径分类
│   ├── thesis_generation.rs    — 标题重叠 + LLM 生成
│   ├── decision_mapping.rs     — RuleEngine + 平滑 + 稳定性
│   ├── loader.rs        — Memory DB 桥接 + load_last_decisions()
│   └── output.rs        — MDX 渲染
├── sulix-observation/  — 数据源适配器 (RSS/Reddit/USPTO) + fetcher + 客户端缓存
└── sulix-cli/          — 入口点: `cargo run -p sulix-cli` | `cargo run --bin intel-pipeline`
    ├── src/main.rs              — 生产管线入口
    ├── src/bin/intel_pipeline.rs — 独立管线调试器
    ├── src/agent/signal.rs       — Signal Agent (抓取 → 去重)
    ├── src/db.rs                — SQLite 去重数据库
    └── src/entity.rs            — EntitySanctionDb (OpenCTI 风格)
```

## 关键架构模式

- **PipelineStep trait**: 统一步骤抽象接口（参考 ripgrep Matcher trait）
- **Builder 模式**: 每步有 XxxStepBuilder 和 build()（参考 ripgrep SearcherBuilder）
- **Fast/Slow 双路径**: 规则分类（零 LLM）或 LLM 语义分类（参考 ripgrep is_line_by_line_fast）
- **PipelineStats**: 运行时间 + 每步数量 + LLM 审计计数器

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
# 每日：cargo run -p sulix-cli --release → R2 → sulix-web 构建 → CF Pages
```

所需 Secrets：
- `DEEPSEEK_API_KEY` — LLM 提供商
- `R2_ACCESS_KEY_ID` / `R2_SECRET_ACCESS_KEY` / `R2_ENDPOINT` — R2 存储
- `CLOUDFLARE_API_TOKEN` / `CLOUDFLARE_ACCOUNT_ID` — Pages 部署

## 许可证

MIT
