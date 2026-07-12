# Sulix Intelligence — Agent Guide

## First Principle

> A decision without evidence is an opinion.
> A decision without falsification is a belief.
> Sulix publishes only assessments that are evidence-backed, traceable, and falsifiable.

Every Thesis output MUST include:
- **Supporting Evidence** (`evidences > 0`) — concrete, verifiable facts
- **Conflicting Evidence** (`challenges` explicitly considered, not just absent)
- **Confidence** (`0.0–1.0`) — updated with each new evidence
- **Falsification Conditions** (`falsification_conditions: Vec<String>`) — 3–5 specific, observable triggers that would weaken or invalidate the thesis
- **Recommended Decision** — actionable direction with horizon and stability
- **Review Trigger** — conditions under which the decision should be re-assessed

Any system output that cannot be traced to evidence and cannot be falsified is rejected.

## Editorial Standards

The following vocabulary rules apply to all LLM prompts and generated content:

- **"Assessment" not "AI output"** — Sulix produces assessments, not AI-generated content
- **"Evidence" not "data"** — each fact must be traceable to a source
- **"Intelligence Signal" not just "signal"** — be precise
- **Evidence Strength not Signal Strength** — use consulting language
- **Never "AI thinks"** — write "Evidence suggests" or "Assessment indicates"
- **Confidence % + evidence count** — always pair confidence with supporting evidence count
- **Falsification required** — every assessment must explicitly state what would invalidate it

See `D:\Project\intel-web\LEXICON.md` for the full vocabulary guide.

---

> Part of three-repository architecture.
> **sulix-engine** owns data acquisition, analysis, memory, and content generation.

## Three-Repository Context

| Repo | Responsibility |
|------|---------------|
| **sulix-engine** (this repo) | Data Acquisition, Analysis, Memory, Content Generation |
| **sulix-web** | Rendering, Navigation, UX |
| **sulix-docs** | Product Decisions, Architecture, ADR, Research |

Cross-boundary changes require ADR.

## Project Structure

```
crates/
├── sulix-contract/     ← 层间契约类型 (Observation/Signal/Thesis/Decision/Theme/Belief/Reflection)
├── sulix-config/       ← TOML 配置加载
├── sulix-llm/          ← LLM Provider (client/retry/api/parser/dispatch/audit)
├── sulix-intelligence/ ← 认知管线 (PipelineStep trait + 3 步骤)
│   ├── step.rs          ← PipelineStep + PipelineStats
│   ├── signal_classification.rs ← Fast/Slow 双路径
│   ├── thesis_generation.rs    ← 匹配 + LLM 生成
│   ├── decision_mapping.rs     ← RuleEngine + 平滑
│   ├── loader.rs        ← Memory DB 桥接
│   └── output.rs        ← MDX 渲染
├── sulix-observation/  ← 数据源适配器 (RSS/Reddit/USPTO)
└── sulix-cli/          ← 命令行入口
    ├── main.rs           ← 生产管线
    ├── bin/intel_pipeline.rs ← 独立调试
    ├── agent/signal.rs   ← Signal Agent
    ├── db.rs             ← SQLite 去重
    └── entity.rs         ← EntitySanctionDb
```

## Commands

```bash
cargo check             # 编译检查
cargo clippy            # lint 检查（保持 0 警告）
cargo test              # 运行测试 (127+ tests)
cargo fmt               # 格式化
cargo fmt --check       # 格式化检查
cargo run -p sulix-cli --release     # 运行完整管线
```

## Output Structure

```
output/
├── daily/       ← 每日信号 MDX (每个 theme 一个文件)
├── thesis/      ← 判断追踪 MDX
├── research/    ← Premium 研报 MDX
└── reflection/  ← 复盘反思 MDX
```

## Data Flow

```
Raw Signals (RSS/USPTO/Reddit)  ← sulix-observation
    ↓
Signal Classification (Fast/Slow Path)  ← sulix-intelligence
    ↓
Thesis Generation (match/LLM)
    ↓
Decision Mapping (RuleEngine + LlmJudge)
    ↓
MDX Output (thesis/{slug}.md + decision/{slug}.md)  ← sulix-cli
```

## DO NOT

- Add UI/rendering logic (CSS, Astro components, frontend frameworks)
- Store generated artifacts (output/) in version control — it's gitignored
- Use local paths like `D:\...` in documentation — use `sulix-engine` / `sulix-web` / `sulix-docs`
- Modify generated pipeline output directly — it's gitignored

## Skill Routing

- `review` — 代码审查
- `qa` — QA 测试
- `ship` — 部署
- `investigate` — 调试问题
