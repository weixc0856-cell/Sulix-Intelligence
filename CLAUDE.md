# Sulix Intelligence — Claude 开发指南

## Core Chain

```
Observation → Signal → Thesis → Decision → MDX
```

Sulix 是一个 Decision Intelligence System。核心管线已全部闭环。

## 项目结构

```
├── Cargo.toml              ← 纯 workspace 根 (无 [package])
│
├── crates/
│   ├── sulix-contract/     ← 层边界类型 (Observation/Signal/Thesis/Decision/Theme/Belief/Reflection)
│   ├── sulix-config/       ← TOML 配置加载
│   ├── sulix-llm/          ← LLM Provider 抽象 (client/retry/api/parser/dispatch/audit)
│   ├── sulix-intelligence/ ← 认知管线 (Signal → Thesis → Decision + PipelineStep trait)
│   ├── sulix-observation/  ← 信号源抓取 (RSS/Reddit/USPTO)
│   └── sulix-cli/          ← 命令行入口 (sulix-cli + intel-pipeline)
│
├── docs/architecture/adr/  ← 架构决策记录
│
├── config.toml             ← 运行时配置
├── config.example.toml     ← 配置示例
└── CLAUDE.md               ← 本文件 (开发指南)
```

## 常用命令

```bash
cargo check --workspace     # 编译检查
cargo clippy --workspace --all-targets --all-features -- -D warnings  # lint（零警告）
cargo test --workspace --all-features  # 运行测试 (113 tests)
cargo fmt                   # 格式化
cargo run -p sulix-cli --release  # 运行完整管线（需 config.toml）
cargo run --bin intel-pipeline  # 独立运行管线测试
```

## 架构原则

- **管线**: Observation → Signal (SignalClassificationStep) → Thesis (ThesisGenerationStep) → Decision (DecisionMappingStep)
- **PipelineStep trait**: 统一步骤抽象（参考 ripgrep Matcher trait 设计），三步通过泛型参数保证类型安全
- **Builder 模式**: 每个 Step 有 XxxStepBuilder（参考 ripgrep SearcherBuilder/RegexMatcherBuilder）
- **Fast/Slow 双路径**: 规则分类（零 LLM）+ LLM 语义分类（参考 ripgrep fast/slow path）
- **contract 层**: 层间契约稳定，Observation 不知 Signal, Intelligence 不知 Memory
- **前端仓库**: [Intel-Web](https://github.com/weixc0856-cell/Intel-Web) (Astro + Content Collections)
- **No dual compilation**: 主入口 `cargo run -p sulix-cli`

## Skill routing

- `review` — 代码审查
- `qa` — QA 测试
- `ship` — 部署
- `investigate` — 调试问题
