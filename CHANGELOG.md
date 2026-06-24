# Changelog

## 2026-06-24

### 架构重构
- **P0**: Source Scoring (triage 加权) + Evidence Snapshot 增强 + Trend Layer 可视化
- **P1**: Memory Engine — Thesis: Evidence[] 信念追踪系统 (`src/engine/memory.rs`)
- **P2**: clusterer 5 文件拆分 (`clustering/analysis/synthesis/llm_prededup`) + change_detection→hermes
- **PremiumEngine**: 研报独立引擎 (`src/engine/premium.rs`)
- **AnalysisEngine**: 分析引擎独立 (`src/engine/analysis.rs`)
- **Hermes 增强**: 趋势检测 + 矛盾写入 + 新 Thesis 发现
- **Thesis Dashboard**: `/memory/` HTML 看板

### 修复
- 429 rate limit 错误地标记为永久错误 → 改为重试
- `last_error.unwrap()` 潜在 panic → safe fallback
- 硬编码 Windows 绝对路径 → config 驱动
- 静默写入失败 x3 → warn 日志
- LLM 去重 JSON 解析静默回退 → warn 日志
- BlueTeam LoopBack 名称 bug (`ClusterNode`→`Cluster`)

### 清理
- 删除未使用模块 `termination.rs` + `versioned.rs` (~600 行)
- 删除 `template.rs` (~300 行死模板)
- 删除死渲染函数 `render_analysis_report`/`render_signal_aggregation`
- 删除死代码实体 normalization + contradiction 计算 (~200 行)
- 删除 5 个 entity.rs 未使用方法
- 删除 4 个 db/catalog/pipeline 死函数
- clippy 警告 35→0, 测试 126→110 (去除死代码对应测试)
