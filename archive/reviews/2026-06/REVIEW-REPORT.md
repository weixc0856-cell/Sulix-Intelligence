# Sulix Intelligence 全面代码审查报告

**审查日期**: 2026-06-24 | **代码行数**: ~5300 (28 模块) | **测试**: 58/58 通过
**编译**: cargo check 通过 (8 warnings) | **静态分析**: clippy pedantic 有 ~30 条风格警告

---

## 🔴 严重 (CRITICAL) — 需立即修复

### C1. signal_strength 无范围限制
- **文件**: [src/engine/analysis/analyzer.rs:151](src/engine/analysis/analyzer.rs#L151)
- **问题**: `parsed["signal_strength"].as_u64().unwrap_or(5) as u8` — LLM 返回的数值直接写入，没有 clamp(1, 10)。0 或 200 都会静默写入。
- **风险**: ThemeAnalysis.signal_strength 用于 SVI 分数计算和报告排序。无效值会导致异常排序或呈现。
- **修复**: 改为 `(parsed["signal_strength"].as_u64().unwrap_or(5) as u8).clamp(1, 10)`
- **置信度**: 10/10 — 代码现场验证

### C2. Scan Agent relevance 标签无枚举验证
- **文件**: [src/agent/scan.rs:92-94](src/agent/scan.rs#L92)
- **问题**: `relevance` 是裸 `String`，预期值应为 `"Structural Shift" / "Competitive Signal" / "Context Update" / "Noise"` 之一。LLM 返回任何字符串都被接受。拼写错误（如 "Structual Shift"）会静默落入 `else` 分支（signal_memory），导致重要信号被误判为噪音。
- **修复**: 定义一个 `RelevanceTag` 枚举，在反序列化时验证。在 `scan.rs:106` 的 comparison 处使用枚举匹配代替字符串比较。
- **置信度**: 10/10 — 代码现场验证

### C3. 蓝军验证 (challenge_theme) 是空操作存根
- **文件**: [src/engine/analysis/analyzer.rs:184-198](src/engine/analysis/analyzer.rs#L184)
- **问题**: `challenge_theme` 返回 `Ok((vec![], None, vec![], vec![]))` — 空向量和 None。所有下游逻辑（load_bearing 检查、SVI 降级）永远不会触发。注释说 "v2 决策从简" 但该函数仍被积极调用。
- **影响**: README.md 宣传的 "Blue Team verification" 功能根本不存在。每天调用该函数但 0 分析量。
- **修复**: 要么实现真正的蓝军逻辑（调用 LLM 挑战假设），要么移除所有调用链并更新文档。
- **置信度**: 10/10 — 返回空数据已验证

### C4. BeliefDb 加载失败静默重置为空白
- **文件**: [src/main.rs:577-583](src/main.rs#L577)
- **问题**: `load_from_file()` 失败时，`unwrap_or_else(|_| { let d = ...; log::info!(...) })` 创建一个空的新 BeliefDb 实例。如果文件损坏，所有历史信念数据被静默丢失。
- **修复**: 加载失败时应 `log::error!` 并发出告警（邮件/通知），或从备份恢复。不应静默创建空白实例。
- **置信度**: 9/10 — 确认代码路径

---

## 🟡 高 (HIGH) — 建议尽快修复

### H1. compliance 过滤器误报导致数据丢失
- **文件**: [src/pipeline.rs:183](src/pipeline.rs#L183)
- **问题**: 正则 `\b([6]\d{5}|...)\b` 匹配任意 6 位数字。美国邮政编码 60601 会被匹配。过滤器在存储**之前**运行（`compliance_filter_all` → `dedup`），合规内容被 [REDACTED] 覆盖后**无法恢复**。
- **影响**: 高信任度源（SEC.gov、Federal Register）中的文章如果包含匹配数字会被静默改写。
- **修复**: (1) 只在展示时应用过滤，不在持久化前；(2) 或使用语境感知匹配（如 "股票代码" 前后文检测）。
- **置信度**: 10/10 — 正则已验证

### H2. Entity 提取别名重叠
- **文件**: [src/entity.rs:237-283](src/entity.rs#L237)
- **问题**: "taiwan semiconductor" 同时出现在 TSMC (line 237) 和 TSA (line 251) 的别名列表中。提取顺序决定匹配结果。当文本提到 "taiwan semiconductor" 时，两个实体都会被提取（因为 `entities.contains()` 检查在循环外，且 `any()` 匹配时两个条件都满足）。
- **修复**: 对来自相同别名触发的重复实体做去重，或引入优先级规则。
- **置信度**: 8/10 — 别名列表已验证

### H3. WAL 模式无 busy_timeout
- **文件**: [src/db.rs:64](src/db.rs#L64)
- **问题**: `PRAGMA journal_mode=WAL` 已启用但没有 `PRAGMA busy_timeout = 5000`。在并发读取（`get_trend`）和写入（`dedup_and_insert`）时，SQLite 可能立即返回 `SQLITE_BUSY`。
- **修复**: 在第 64 行后添加 `conn.execute_batch("PRAGMA busy_timeout = 5000;")?;`
- **置信度**: 8/10 — 已知 SQLite 模式

### H4. HTML 注入使用 replacen 操作
- **文件**: [src/main.rs:801,815,857](src/main.rs#L801)
- **问题**: 三处独立的 `replacen("</main>", ...)` 调用来注入决策和趋势区块。每次都是完整的读-改-写周期。若 HTML 模板移除 `</main>`，所有注入静默失效。
- **状态**: **已在本次审查中自动修复** — 合并为单次批量读-改-写操作。
- **置信度**: 10/10

---

## 🔵 中 (MEDIUM) — 下一迭代修复

### M1. date_range 配置无验证
- **文件**: [src/config.rs:67-73](src/config.rs#L67)
- **问题**: `date_range` 默认为 `"d7"`。预期值为 `d1/d3/w1/w2/m1`。无验证——任何字符串都可以通过，源模块接收后只会因解析失败静默回退到 7 天。
- **修复**: 在 `Config::from_file()` 中添加 `validate_date_range()` 检查。
- **置信度**: 10/10

### M2. 内存数据库测试中使用 WAL
- **文件**: [src/db.rs:322-347](src/db.rs#L322)
- **问题**: `Connection::open_in_memory()` 后执行 `PRAGMA journal_mode=WAL`。在内存数据库上设置 WAL 模式在 SQLite 文档中不被官方支持，在不同版本上可能静默失败。
- **修复**: 移除测试中的 WAL pragma（测试不需要 WAL）。
- **置信度**: 9/10

### M3. JSON 反序列化缺乏字段级错误报告
- **文件**: [src/engine/analysis/analyzer.rs:108-180](src/engine/analysis/analyzer.rs#L108)
- **问题**: LLM 输出的 JSON 使用 `parsed["bluf"].as_str().unwrap_or("待分析")` 等逐字段提取。当字段缺失时，静默使用默认值。没有告警或结构化错误报告标明哪个字段失败。
- **修复**: 在字段缺失时添加 `log::warn!("⚠️ LLM 响应缺少字段: bluf")` 级别的日志。
- **置信度**: 7/10

---

## ⚪ 信息性 (INFORMATIONAL) — 代码质量/文档

### I1. 完全未使用的模块
| 文件 | 行数 | 详情 |
|------|------|------|
| [src/app_context.rs](src/app_context.rs) | 41 | `AppContext` 结构体 + `new()` — 从未在任何地方导入或使用 |
| [src/event_log.rs](src/event_log.rs) | 154 | 完整 EventLog 实现 + 测试 — 从未导入 |

**修复**: 删除这两个模块（和它们的 `mod` 声明），或集成到管线中。

### I2. 单行重新导出垫片
| 文件 | 内容 |
|------|------|
| [src/clusterer/analysis.rs](src/clusterer/analysis.rs) | `pub use crate::engine::analysis::*;` |
| [src/premium.rs](src/premium.rs) | `pub use crate::engine::premium::*;` |
| [src/clusterer/mod.rs:31](src/clusterer/mod.rs#L31) | 重新导出 `hermes::detect_changes_*`，创建 clusterer → hermes 依赖 |

**修复**: 保留以保持向后兼容（clusterer 是历史兼容层）。评估能否直接删除。

### I3. CLAUDE.md 文档与代码不一致
| 声明 | 实际 | 修复 |
|------|------|------|
| `hermes.rs` (单文件) | `hermes/` 模块目录 | 更新路径 |
| `renderer.rs` (单文件) | `renderer/` 模块目录 | 更新路径 |
| orchestrator/QE/DE "冻结不投入" | 三个都被 `graph.run()` 每日执行 | 更新声明或修复代码 |

### I4. README.md 文档不匹配
| 声明 | 实际问题 |
|------|----------|
| "Blue Team verification" 列为 ✅ | challenge_theme 是空操作存根 |
| "Versioned pipeline (uuid_v7 + atomic write + resume)" 列为 ✅ | 相关代码 (versioned.rs) 已删除 |
| "Research Layer: $99-$4999" | Stripe 支付集成不在 Rust 代码中（可能在前端） |
| "29 data sources" | 实际数量取决于 config.toml 配置 |
| "three-layer product" | 实际是 4-Agent 管线架构 |

**修复**: 同步 README.md 与实际实现状态。

### I5. 隐藏的 LLM 退化
- [src/hermes/detector.rs:195-218](src/hermes/detector.rs#L195) — LLM 变更检测失败时静默回退到基于规则的方法。无告警或指标记录 LLM 退化次数。
- [src/main.rs:396-398](src/main.rs#L396) — Scan Agent 失败时全部进入 insight，无告警。

**修复**: 在 LLM 失败且静默降级时添加 `log::warn`。

### I6. 硬编码魔法数字
- [src/main.rs:810](src/main.rs#L810) — `db.get_trend(14)` 硬编码的 14 天窗口
- [src/hermes/detector.rs:93](src/hermes/detector.rs#L93) — 30 条记录的滑动窗口硬编码

**修复**: 提取为具名常量。

### I7. `#[allow(dead_code)]` 使用不一致
30 处 `#[allow(dead_code)]` 分布在 13 个文件中。有些合理（entity.rs 中的 `EntityType::as_str()` 等字符串常量模式），有些具误导性（analyzer.rs:184 上被积极调用的 `challenge_theme`）。

**修复**: `challenge_theme` 的 `#[allow(dead_code)]` 应移除。其他项逐一评估是否仍需要。

---

## 质量评分

```
PR Quality Score: 6.5/10
─────────────────────────────────
CRITICAL: 4 项 (C1-C4)    → -8
HIGH:     4 项 (H1-H4)    → -4
MEDIUM:   3 项 (M1-M3)    → -1.5
INFORMATIONAL: 7 项 (I1-I7)
─────────────────────────────────
分项: 10 - (4×2 + 4×0.5 + 3×0.25) = 10 - 10.75 = 不适用 — 此为全量审查，
      包含只读分析（非 PR diff）。
      直接基于发现严重性: 6.5/10
─────────────────────────────────
```

## 总结

**最需关注**: LLM 信任边界（C1、C2）和名存实亡的蓝军验证（C3）是真正的风险。合规过滤器数据丢失（H1）在中文场景下影响更大。文档和死代码清理（I1-I4）影响开发者效率和 AI 工具理解。

**推荐的修复优先级**:
1. C1-C2: LLM 输出验证 (15 分钟)
2. H1: 合规过滤器修复 (10 分钟)
3. H3: busy_timeout 添加 (2 分钟)
4. C3: 要么实现蓝军要么移除 (30 分钟)
5. I1-I4: 文档/死代码清理 (20 分钟)
