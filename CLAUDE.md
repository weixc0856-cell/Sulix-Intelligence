# Sulix Intelligence — Claude 开发指南

## 项目结构

```
src/
├── engine/           ← 核心引擎（analysis / memory / premium）
├── clusterer/        ← 聚类 + 共享类型（重导出入口）
├── hermes.rs         ← 变更检测 + 趋势/矛盾/Thesis 发现
├── renderer.rs       ← HTML + Markdown 渲染
├── main.rs           ← 4-agent 管线编排
└── ...               ← 基础设施（config/db/llm/fetcher 等）
```

## 常用命令

```bash
cargo check             # 编译检查
cargo clippy            # lint 检查（保持 0 警告）
cargo test              # 运行测试
cargo fmt               # 格式化
cargo run --release     # 运行完整管线
```

## 架构原则

- **4-Agent 管线**: init → agent_signal → agent_research → agent_publish
- **engine/ 目录**: 核心领域逻辑，独立于管线编排
- **clusterer/ 重导**: 历史兼容层，新代码直接引用 engine 子模块
- **死代码冻结**: orchestrator/question_engine/decision_engine 冻结不投入

## Skill routing

- `review` — 代码审查
- `qa` — QA 测试
- `ship` — 部署
- `investigate` — 调试问题
