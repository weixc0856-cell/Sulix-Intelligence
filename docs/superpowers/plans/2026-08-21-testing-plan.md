# Testing Plan (Sprint 6.5 — 测试体系补全与门禁强化)

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development (recommended) to implement this plan task-by-task.

**Goal:** 在架构去耦（见 `2026-08-21-architecture-decoupling-plan.md`）进行的同时，补齐测试体系：修复测试基线、补零测试层的覆盖、建立覆盖率与依赖方向门禁、建立分层测试策略，让"每域迁移必有测试护航"成为硬约束。

**Tech Stack:** Rust 标准 `cargo test`（无 tokio/mockall 依赖，纯同步 + async-trait `?Send`）+ MemoryStore 内存后端。新增工具：`cargo-llvm-cov`（覆盖率）。

---

## 一、测试现状盘点（2026-08-21 实测）

### 数据基线

| 指标 | 当前值 | FULL_REVIEW_REPORT 基线 (2026-06-22) | 说明 |
|------|--------|------|------|
| `#[test]` 总数 | **289** | 255 | 6 周 +34 |
| 可测试 crate | 25 | 25 | — |
| 零测试 crate | 0 | 4 (events, infrastructure, shared-kernel, vectorize) | 已补一部分 |
| 覆盖率工具 | 无 | 无 | 完全空白 |
| 集成测试 | 0 | 0 | `crates/store/tests/` 目录存在但**为空** |
| CI 测试门禁 | `cargo test`（无覆盖/无 wasm 验证） | 同 | — |
| clippy / fmt | — | 12 warnings / 16 文件未格式化 | 门禁当前是红的 |

### 测试分布（前 8）

```
store 48 · signal-engine 31 · reasoning-framework 23 · decision-engine 22
ai-pipeline 18 · fetcher 16 · entity 13 · rules 13 · context-engine 10
```

### 现状问题（按严重度）

1. **`infrastructure` 适配器层 0 测试** —— 去耦计划 Phase 3 将把大量 SQL 映射迁到这里，当前完全裸奔。最危险。
2. **无覆盖率工具与门禁** —— 无法阻止"迁移删代码时顺手删测试"。
3. **无集成测试** —— `store/tests/` 空目录；跨域流转（observe → claim → signal → decision）无端到端验证。
4. **CI 门禁是红的** —— clippy 12 warnings、fmt 16 文件失败，导致 `-D warnings` 门禁形同虚设（lint.yml 里 `cargo clippy -- -D warnings` 应已失败，但 PR 仍合入）。
5. **PR 无 wasm 构建验证** —— wasm32 check 只在 deploy.yml（push 后）执行，合入前不可见。
6. **delivery 层测试薄弱** —— api 9 个、worker-entry 5 个，且都只测纯逻辑；cron/queue 调度、handler 解析/转换逻辑无测。
7. **LLM 相关逻辑测试稀疏** —— model-runtime 9 个（已有 `MockProvider` 模式），claim-engine extractor 依赖真实 LLM 调用无 mock 测试。
8. **基线已过时** —— FULL_REVIEW_REPORT §5 记录 255 测试，实际 289；需更新并做趋势跟踪。

### 既有正确模式（保持并推广）

- **MemoryStore 注入**：store 有完整内存实现，应用层/适配器层可用它做伪后端 → 这是测试基础设施的基石，缺的只是"用它写测试"。
- **纯逻辑分离**：rules/search/entity/decision-engine 已是纯函数式 crate，测试质量高（决策引擎 22 个）。
- **MockProvider**：model-runtime/router.rs 已有 mock 提供者先例。
- **线上 smoke test**：deploy.yml 已有 health + semantic search 冒烟。

---

## 二、分层测试策略（测试金字塔）

| 层 | 测试类型 | 手段 | 现状 | 目标 |
|----|---------|------|------|------|
| 纯逻辑层（rules, search, entity, shared-kernel, decision-engine, reasoning-framework, intelligence-domain） | 单元测试 | 纯同步 assert | ✅ 强 | 覆盖率 ≥ 80% |
| 应用层（application） | 用例测试 | 注入 MemoryStore/伪端口，async-trait | 🟡 6 个，偏少 | 每个 UseCase 至少 1 组正常+异常路径 |
| 基础设施适配器（infrastructure, store::domain CRUD） | 适配器映射测试 | MemoryStore-backed 伪实现；SQL 正确性交 miniflare | 🔴 0 个 | 每个适配器 1:1 覆盖 save/find/delete 映射 |
| delivery（api handlers, worker-entry runtime） | 纯逻辑抽取测试 | 把解析/转换/调度决策抽成纯函数测 | 🟡 薄弱 | 每个 handler 的抽取逻辑覆盖 |
| 跨域集成（observe→claim→signal→decision→reflection） | 集成测试 | MemoryStore 全链路，无网络 | 🔴 0 个 | 1 条主链路 + 每域 1 条 |
| E2E（线上 Worker） | 冒烟 | deploy.yml curl | 🟡 2 个端点 | 增加 pipeline/status 等关键端点 |

**原则：**
- 不引入 tokio 运行时、不引入 mockall —— 保持"纯同步 + ?Send async + 内存后端"的轻量模式，这是当前 289 测试能在 CI 快速跑完的原因。
- wasm 相关行为（D1 SQL 真实执行、R2、Workers AI）**不**在 host 单测中模拟，交给：a) 适配器映射测试（逻辑层）b) miniflare 本地集成（P2，可选）c) 线上 smoke test。
- 每个去耦任务（见去耦计划）的验收标准之一 = 该域迁移后的测试数 ≥ 迁移前。

---

## 三、Task Plan

### Task T1: 修复基线 —— 让门禁重新可执行（前置，所有其他任务依赖）

**Files:**
- 16 个未格式化文件：`cargo fmt` 全仓
- Modify: `crates/reasoning-framework/src/framework.rs`、`seed.rs`、`calibration.rs`、`lib.rs`、`repository.rs`、`selector.rs`（clippy 7 项）
- Modify: `crates/model-runtime/src/gateway.rs`（clippy 2 项）
- Modify: `crates/decision-engine/src/aggregate.rs`（too_many_arguments，用 builder/struct 收参数）
- Modify: `crates/intelligence-domain/src/engine.rs`、`signal.rs`（dead_code：claims/signals/SignalInstance —— 去耦计划 Phase 6 会处理，此处先加 `#[allow(dead_code)]` 或删）
- Modify: `crates/fetcher/Cargo.toml`、`crates/vectorize/Cargo.toml`（删除 unused deps：uuid、wasm-bindgen-futures）

**Actions:**
1. `cargo fmt` 全仓格式化并提交。
2. 按 FULL_REVIEW_REPORT §1 的逐条建议清零 12 个 clippy warnings。
3. 删除 2 个 unused deps。
4. 更新 `FULL_REVIEW_REPORT.md` §1 与 §5（新基线）。

**Tests:** `cargo clippy --workspace --all-targets -- -D warnings` 绿；`cargo fmt --check` 绿；`cargo test --workspace` 全绿。

**Commit:** `chore: fix formatting baseline` / `refactor: clear clippy warnings` / `chore: remove unused dependencies`

### Task T2: 补齐 infrastructure 适配器测试（最高优先，去耦 Phase 3 前必做）

**Files:**
- Create: `crates/infrastructure/src/decision_repository.rs` 内 `#[cfg(test)] mod tests`（或独立 `crates/infrastructure/tests/decision_repository.rs`）
- Create: `crates/infrastructure/src/{artifact_registry,provenance,storage_policy}.rs` 测试
- Modify: `crates/infrastructure/Cargo.toml`（dev-deps：`store` 已有，MemoryStore 可用）

**Actions:**
- 用 `store::MemoryStore` 实现 `DecisionRepository` 的伪后端场景：`D1DecisionRepository<MemoryStore>` —— 验证 status 映射（Draft→draft 等 6 项）、save/find 往返、find_by_signal 过滤、list 状态过滤。
- 后续每个新适配器（去耦 Phase 3）创建时**必须**同结构带测试。

**Tests:** 适配器 save/find/list 往返、状态枚举 1:1 映射表驱动测试、错误路径（缺失行 → None）。

**Commit:** `test(infrastructure): decision repository mapping tests`（每适配器一个 commit）

### Task T3: 补 shared-kernel / events 契约测试

**Files:**
- Create: `crates/shared-kernel/src/ids.rs` 测试（ID 格式：OBS-000001 等）
- Create: `crates/shared-kernel/src/events.rs` 测试（事件 schema 序列化往返、变体字段完整性）
- Create: `crates/shared-kernel/src/lineage_query.rs`、`time.rs` 测试

**Actions:**
- 契约测试：`DecisionDomainEvent` 各变体 serde 往返；ID newtype 格式化与解析（如有）。
- 这些是跨域公共契约，破坏即全仓连锁 —— 必须有测试锁。

**Tests:** 序列化/反序列化往返、格式化边界（0、超大 id）。

**Commit:** `test(shared-kernel): id and event contract tests`

### Task T4: 引入覆盖率工具 + 门禁

**Files:**
- Create: `.github/workflows/coverage.yml`
- Modify: `.github/workflows/lint.yml`
- Create: `docs/testing.md`（覆盖率解读与例外规则）

**Actions:**
1. 安装 `cargo-llvm-cov`（workspace 根 `cargo install` 或 CI `taiki-e/install-action`）。
2. 对**纯逻辑层 + 应用层**的 crate 生成覆盖率（wasm 相关 crate 排除或只统计非 wasm 模块——D1 SQL 无法在 host 覆盖）。
3. 初始阈值：**纯逻辑层 ≥ 70%，整体 ≥ 50%**（起步宽松，每 sprint +5%）；低于阈值 CI 失败。
4. 覆盖率报告上传（actions/upload-artifact），并在 PR 注释展示 delta。

**Tests:** 本地 `cargo llvm-cov --workspace` 跑通；CI job 绿色。

**Commit:** `ci: add coverage gate with cargo-llvm-cov` / `docs: coverage policy`

### Task T5: CI 强化 —— PR 门禁补全

**Files:**
- Modify: `.github/workflows/lint.yml`
- Modify: `.github/workflows/deploy.yml`

**Actions:**
1. lint.yml 增加 wasm 兼容检查步骤（`cargo check --workspace --all-targets --all-features` + `rustup target add wasm32-unknown-unknown`），让 PR 合入前即可见 wasm 编译错误（当前只在 push 后 deploy 时暴露）。
2. lint.yml 增加 `cargo test --workspace` 的分 crate 输出汇总（`--message-format` 或 per-crate job），失败时能定位是哪个 crate。
3. deploy.yml 保持；smoke test 增加 `/api/pipeline/status` 端点（可选项，P2）。
4. 架构护栏 job（见去耦计划 Phase 7 T7.1）。

**Tests:** 触发一个 PR 验证所有 job 通过。

**Commit:** `ci: add wasm check to PR gate`

### Task T6: 应用层用例测试补强（与去耦 Phase 5 联动）

**Files:**
- Create: `crates/application/tests/` 或各 service 内 `#[cfg(test)]`
  - `decision_service.rs`：propose 正常流、事件排空、非法状态拒绝
  - `signal_service.rs`（Phase 5 新增后）：信号聚合 → 持久化 → 事件
  - `graph.rs` / `radar.rs`：现有 2 个服务的边界补测

**Actions:**
- 每个 UseCase：正常路径 + 至少一个失败路径（不变量违反、存储错误）。
- 用 `MemoryStore` + 伪 `ArtifactRegistry` 注入（`ArtifactRegistry` trait 已有 MemoryStore 对应实现吗？无则先补一个 `MemoryArtifactRegistry` 测试用实现——放 `crates/application/tests/common/` 或 store crate 的测试支持模块）。

**Commit:** `test(application): use-case coverage for decision/signal services`

### Task T7: 去耦联动测试（每个去耦 Task 的测试护航）

与 `2026-08-21-architecture-decoupling-plan.md` 逐 Task 对应：

| 去耦 Task | 测试要求 |
|-----------|---------|
| P2 端口定义 | 端口需求清单表（signal-engine 对 store 的每个调用 ↔ 端口方法）写成测试枚举 |
| P3 适配器迁移 | 每个新适配器带 T2 风格映射测试；被迁移的 `store::domain` 原测试迁移到新适配器（**删除旧测试=红灯**） |
| P4 收缩 StoreBackend | 每删一组方法：全仓测试绿 + 编译失败即检测到未迁移调用方 |
| P5 应用层补位 | 每个新 service 按 T6 标准补测试；被搬移逻辑的原测试移到 service |
| P6 清理旧 crate | 迁移完成后旧 crate 测试删除时必须已在新位置 1:1 存在（用 grep 计数核对） |

**硬约束：每个去耦 commit 的测试数 ≥ 该域迁移前测试数。** 在 CI 中通过一个简单脚本比较 `cargo test` 统计（可暂用 `grep -c '#[test]'` 于 PR diff 的 crate）。

**Commit:** 随各去耦 commit 一起，不单独提交。

### Task T8: 集成测试 —— 跨域主链路（去耦 Phase 4 后执行，依赖端口可用）

**Files:**
- Create: `crates/application/tests/end_to_end.rs`（或 `crates/store/tests/` 重新启用——推荐 application/tests，避免 store 再次膨胀）

**Actions:**
- 用全内存后端跑通主链路：`observe(article) → claim(提取+置信度) → signal(聚合+生命周期) → decision(提案+状态机+outcome) → reflection(复盘)`。
- 每步断言：状态正确、事件累积正确、ID 引用一致（OBS→SIG→DEC 链）。
- 这是去耦后的"总装测试"——替代当前缺失的 store/tests。

**Tests:** 主链路 1 条 + 每域 1 条局部链（如 claim→signal）。

**Commit:** `test(application): end-to-end cognitive loop integration`

### Task T9: delivery 层测试补强

**Files:**
- Modify: `crates/api/src/routes/*.rs`（抽取纯函数：query 参数解析、响应构建、错误映射）
- Modify: `crates/worker-entry/src/runtime/cron.rs`、`queue.rs`（抽取：任务分发决策、job 类型解析为纯函数）
- Create: 对应 `#[cfg(test)]` 模块

**Actions:**
- handler 内"解析 + 转换"逻辑抽纯函数并测（JSON 非法、参数缺省、错误码映射）。
- cron/queue 的调度决策（哪个 job、哪些 feed、重试策略）抽纯函数测。

**Commit:** `test(api): handler parsing/response tests` / `test(worker-entry): cron/queue dispatch tests`

### Task T10: 基线更新与趋势跟踪（收尾，每 sprint 复查）

**Files:**
- Modify: `FULL_REVIEW_REPORT.md` §5（新基线：测试数、覆盖率、clippy、fmt、Tech Debt）
- Create: `docs/testing.md`（测试策略、覆盖率政策、per-crate 现状表）

**Actions:**
- 基线表增加：`Coverage (core)`、`Integration Tests`、`CI Jobs` 三项。
- 每 sprint 末更新一次，作为趋势追踪。

**Commit:** `docs: update testing baseline and policy`

---

## 四、验收标准（DoD）

1. `cargo test --workspace --all-features` 全绿，总数 ≥ 320（289 + 去耦新增约 30+）。
2. `infrastructure` 适配器层测试从 0 → 每适配器 ≥ 1 组映射测试。
3. 覆盖率门禁生效：纯逻辑层 ≥ 70%；CI 有 coverage job。
4. 集成测试 ≥ 1 条跨域主链路。
5. 每个去耦 commit 测试数不降（CI 脚本核对）。
6. clippy/fmt/deny 门禁全绿（T1 修复后不再回归）。
7. FULL_REVIEW_REPORT §5 基线已更新。
8. 无新增第三方测试依赖（保持轻量模式）。

---

## 五、执行顺序与依赖

```
T1（基线修复，所有任务前提）
 ├─ T4（覆盖率） ── T5（CI 强化）── T2/T3（基础层测试）── T6/T7（随去耦）── T8（集成）── T9（delivery）
 └─ 与去耦计划并行：T2 在去耦 Phase 3 前；T7 是去耦每个 commit 的硬约束
```

**建议节奏：** T1→T2→T3→T4→T5 为第一波（1 个 sprint）；T6→T7 随去耦各阶段进行；T8→T9→T10 为第二波。
