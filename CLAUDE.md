# Sulix Intelligence — Claude 开发指南

## 项目结构

```
src/
├── domain/           ← 7 个领域模型 (Theme/Thesis/Evidence/Observation/Action/Outcome/Reflection)
├── engine/           ← 核心引擎 (analysis/memory/premium/belief)
├── hermes/           ← 变更检测 + 趋势/矛盾/Thesis 发现
├── renderer/         ← MDX 输出 (主要) + HTML/Markdown (次要)
│   ├── mdx.rs        ← ✅ MDX 知识资产渲染器 (主要输出格式)
│   ├── publisher.rs  ← Publisher trait + MdxPublisher + HtmlPublisher
│   └── ...
├── agent/            ← Scan Agent + Editor Agent + Calibration + Decay
├── clusterer/        ← 聚类 + 共享类型
├── source/           ← 源适配器 (RSS/USPTO/Reddit)
├── twitter.rs        ← Twitter/X 推文管线
├── publishing.rs     ← Publishing Agent 编排
├── event_log.rs      ← PipelineEvent 审计日志
└── main.rs           ← 管线编排 (629 行)
```

## 常用命令

```bash
cargo check             # 编译检查
cargo clippy            # lint 检查（保持 0 警告）
cargo test              # 运行测试 (127+ tests)
cargo fmt               # 格式化
cargo run --release     # 运行完整管线
```

## 架构原则

- **4-Agent 管线**: init → agent_signal → agent_research → agent_publish
- **MDX 是主要输出格式**: engine 生成 MDX 知识资产，前端在单独仓库渲染
- **前端仓库**: [Intel-Web](https://github.com/weixc0856-cell/Intel-Web) (Astro + Content Collections)
- **engine/ 目录**: 核心领域逻辑，独立于管线编排
- **认知引擎**: orchestrator (DiGraph) + question_engine + decision_engine 构成認知管线

## 输出结构

```
output/
├── daily/       ← 每日信号 MDX (每个 theme 一个文件)
├── thesis/      ← 判断追踪 MDX
├── research/    ← Premium 研报 MDX
└── memory/      ← 复盘反思 MDX
```

## Skill routing

- `review` — 代码审查
- `qa` — QA 测试
- `ship` — 部署
- `investigate` — 调试问题
