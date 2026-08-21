# Architecture Decoupling Plan (Sprint 6.5 — Store God-Object Demolition)

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development (recommended) to implement this plan task-by-task.

**Goal:** 拆除 `store` 上帝对象与中间层 crates 对基础设施的直接耦合，让 DDD 分层（domain / application / infrastructure / delivery）真正生效，并消除 `intelligence-domain` 的"伪迁移"。

**Architecture:** 目标态为严格单向依赖：

```
delivery (api + worker-entry)
      ↓ 只依赖 application + 领域类型 + worker
application（用例服务，泛型注入端口）
      ↓
domain（intelligence-domain / decision-engine / reasoning-framework / shared-kernel）
      ↑ 端口（Repository traits 定义在领域层）
infrastructure（D1 适配器，实现领域 trait）──→ store（仅作为 D1 数据访问库）
```

**Tech Stack:** Rust + Cloudflare Workers + D1。`store` 从"业务仓库超集"降级为纯 D1 数据访问层。

**Spec reference:** `FULL_REVIEW_REPORT.md` §3 (Architecture Review)、§5 (Baseline: Health 2.5/10, Tech Debt 10)

**关联计划:** 每个 Task 的测试要求见 `docs/superpowers/plans/2026-08-21-testing-plan.md`（T7 与本文档联动）。

---

## 背景：已确认的三个耦合问题

1. **`store` 是上帝对象**（5800 行，13 个 crate 直接依赖，31 处 `use worker`）。其 `domain/` 目录实为 D1 数据访问层（[store/src/domain/decision/outcome.rs:8](crates/store/src/domain/decision/outcome.rs#L8) 直接 `use worker::wasm_bindgen::JsValue`），`StoreBackend` supertrait（500 行）虽已标注 deprecated 仍是事实总入口。
2. **`api`/`worker-entry` 是胖 delivery 层**（分别依赖 15/16 个内部 crate，`api` 中 `use store::` 21 处），绕过 `application` 直触基础设施；`application` 仅有 2 个用例被消费。
3. **`intelligence-domain` 伪迁移**：领域类型已搬入新 crate，但代码层零 `use intelligence_domain::` 调用，`signal-engine`/`claim-engine` 死而不僵。

**健康项（不得破坏）：** 无循环依赖；`shared-kernel` 唯一公共底座；`infrastructure` 适配器方向正确（[infrastructure/src/decision_repository.rs](crates/infrastructure/src/decision_repository.rs)）。

---

## 目标态依赖图

```
                    ┌────────────── shared-kernel ◄─────────────┐
                    │              （纯值对象/ID/事件）            │
   domain           decision-engine ◄────── application ────────┤
                    intelligence-domain ◄── (signal/claim 消费方) │
                    reasoning-framework ─────────────────────────┤
   application      application/services/*（唯一用例入口）───────► store（仅数据访问）
   infrastructure   infrastructure/src/{decision,signal,claim,reflection,memory}_repository.rs
                    （实现领域 trait；依赖 store 做 D1 映射）
   delivery         api（只依赖 application + domain 类型 + worker）
                    worker-entry（只依赖 api + infrastructure + worker）
```

---

## Phase 1 — 封死蔓延（止血，立即执行）

### Task 1.1: 依赖门禁（cargo-deny bans）

**Files:**
- Modify: `deny.toml`
- Modify: `Cargo.toml`（workspace，如需增加 `[workspace.metadata]` 说明）

**Actions:**
- 在 `deny.toml` 的 `[bans]` 增加规则：禁止 `store`、`vectorize`、`embedding`、`event-store`、`object-store` 作为**新增**依赖进入非 `infrastructure`/`api`/`worker-entry`/`store` 之外的 crate（对遗留 crate 用 allow 名单豁免，写明到期 sprint）。
- 目标清单（受控 crate）：`signal-engine`、`reflection-engine`、`memory-engine`、`ai-pipeline`、`context-engine`、`agent-engine`、`claim-engine`。

**Tests:** `cargo deny check bans` 通过；新增一个违规依赖时 CI 必须失败。

**Commit:** `ops(governance): ban store/vectorize/embedding deps outside infra layer`

### Task 1.2: 门禁接入 CI 与治理文档

**Files:**
- Modify: `.github/workflows/lint.yml`（`cargo deny check bans` 已存在，确认新规则生效）
- Modify: `CLAUDE.md`（架构治理一节补充依赖白名单说明）

**Tests:** 空库提交验证 CI lint 通过。

**Commit:** `docs(governance): document dependency whitelist policy`

---

## Phase 2 — 补齐领域端口（domain-owned repository traits）

原则：每个业务上下文在**自己的领域 crate** 定义 Repository trait，方法签名只用领域类型（参考 [decision-engine/src/repository.rs](crates/decision-engine/src/repository.rs) 范本）。

### Task 2.1: Intelligence 域端口审计与补全

**Files:**
- Modify: `crates/intelligence-domain/src/repositories.rs`（已有 Observation/Claim/Signal 三端口）
- Create: `crates/intelligence-domain/src/repositories.rs` 追加 `SignalEvidenceRepository`（覆盖 `store::SignalEvidenceSummary`、`SignalMutation` 等 signal-engine 实际使用的数据操作）
- Create: `crates/reflection-domain`（若决定反射域独立）或并入 intelligence-domain

**Actions:**
- 把 [signal-engine](crates/intelligence/signal-engine/src) 中对 `store::*`、`vectorize::*`、`embedding::*` 的每一处调用（已 grep 到 `SignalMutation`/`SignalEvidenceSummary`/`EntitySignalRef`/`VectorizeIndex::query` 等）整理成端口需求清单。
- 端口方法签名只暴露领域类型，禁止 `JsValue`/SQL 字符串。

**Tests:** 端口 trait 编译通过；端口清单与 signal-engine 实际调用一一对应（写成测试枚举需求表）。

**Commit:** `feat(intelligence-domain): add signal evidence repository port`

### Task 2.2: Reflection / Memory 域端口

**Files:**
- Create: `crates/reflection-engine/src/repository.rs`（ReflectionRepository）
- Create: `crates/memory-engine/src/repository.rs`（MemoryRepository）
- Modify: 两个 crate 的 Cargo.toml（如需要，减少对 store 的直接依赖范围）

**Actions:**
- 分析 [reflection-engine](crates/intelligence/reflection-engine/src) 与 [memory-engine](crates/memory-engine/src) 对 `store`/`event-store` 的调用，定义领域端口。
- `event-store` 调用收敛到 `EventPublisher` 端口（领域层定义，infrastructure 实现）。

**Commit:** `feat(reflection-engine): domain-owned repository port` / `feat(memory-engine): domain-owned repository port`

---

## Phase 3 — 迁移适配器到 infrastructure

范本：[infrastructure/src/decision_repository.rs](crates/infrastructure/src/decision_repository.rs)（`D1DecisionRepository<S: StoreBackend>` 实现 `decision_engine::DecisionRepository`）。

### Task 3.1: Signal / Observation / Claim 适配器

**Files:**
- Create: `crates/infrastructure/src/signal_repository.rs`
- Create: `crates/infrastructure/src/observation_repository.rs`
- Create: `crates/infrastructure/src/claim_repository.rs`
- Modify: `crates/infrastructure/src/lib.rs`（导出）

**Actions:**
- 实现 `intelligence_domain::{SignalRepository, ObservationRepository, ClaimRepository, SignalEvidenceRepository}`。
- D1 SQL 映射从 `store/src/domain/{signal,observation,claim}/*` 搬迁/委托过来；`store` 中原实现标记 deprecated。

**Tests:** 用 `MemoryStore`（已存在）实现同端口伪后端，做适配器映射测试（见测试计划 T2）。

**Commit:** `feat(infrastructure): D1 adapters for intelligence domain ports`

### Task 3.2: Reflection / Memory 适配器

**Files:**
- Create: `crates/infrastructure/src/reflection_repository.rs`
- Create: `crates/infrastructure/src/memory_repository.rs`
- Modify: `crates/infrastructure/src/lib.rs`

**Tests:** 同上，MemoryStore-backed。

**Commit:** `feat(infrastructure): D1 adapters for reflection/memory ports`

### Task 3.3: 组装点注入

**Files:**
- Modify: `crates/worker-entry/src/lib.rs`（或 `runtime/` 下统一组装模块）
- Modify: `crates/worker-entry/src/runtime/*.rs`

**Actions:**
- `fetch`/`scheduled`/`queue` 入口统一从 `Env` 构造 D1 → 各适配器 → 注入 application 服务。删除各 handler 内直接 `Store::new(ctx.env.d1("DB"))` 的散点。

**Commit:** `refactor(worker-entry): compose adapters at entry point`

---

## Phase 4 — 收缩 StoreBackend

### Task 4.1: 逐域删除方法

**Files:**
- Modify: `crates/store/src/backend.rs`
- Modify: `crates/store/src/d1_delegate.rs`
- Modify: `crates/store/src/memory/backend.rs`

**Actions:**
- 每迁移一个领域（Phase 3 完成者），删除 `StoreBackend` 对应方法组 + D1 delegate 转发。
- 保留 thin 兼容层（`#[deprecated]`）直到 Phase 7 完全删除 supertrait。

**Tests:** 每删一组方法，`cargo test --workspace` 全绿；被删除方法的旧调用方必须已迁移（编译失败即检测）。

**Commit:** `refactor(store): remove signal methods from StoreBackend`（每域一个 commit）

---

## Phase 5 — 应用层补位（application 成为唯一用例入口）

### Task 5.1: 编排逻辑上收

**Files:**
- Create: `crates/application/src/services/signal.rs`（SignalService）
- Create: `crates/application/src/services/reflection.rs`（ReflectionService）
- Create: `crates/application/src/services/ingestion.rs`（AI pipeline 编排）
- Modify: `crates/application/src/services/mod.rs`、`crates/application/src/lib.rs`

**Actions:**
- 把 [api/src/routes/signal.rs](crates/api/src/routes/signal.rs)、[worker-entry/src/runtime/intelligence.rs](crates/worker-entry/src/runtime/intelligence.rs) 等处的编排逻辑（SignalEngine 调用、事件排空、LLM 重试）搬入 application 服务。
- 服务泛型化注入端口（`S: SignalRepository + ...`），零 HTTP/Worker 代码，可用 MemoryStore 单测（保持 [application/src/lib.rs](crates/application/src/lib.rs) 既有承诺）。

**Tests:** 每个新 service 用内存端口写用例测试（见测试计划 T6）。

**Commit:** `feat(application): signal/reflection/ingestion use-case services`

### Task 5.2: delivery 层改为调用 application

**Files:**
- Modify: `crates/api/src/routes/*.rs`（signal、decision、reflection、semantic 等）
- Modify: `crates/worker-entry/src/runtime/*.rs`
- Modify: `crates/api/Cargo.toml`、`crates/worker-entry/Cargo.toml`

**Actions:**
- handlers 只做：解析 HTTP → 构造命令 → 调 application service → 转 JSON。
- 目标态 `api` 依赖面：`worker` + `application` + 领域类型 + `serde`；删除对 `vectorize`/`embedding`/`event-store`/`object-store`/`infrastructure` 的直接依赖（组装留给 worker-entry）。
- `worker-entry` 依赖面收敛为：`api` + `infrastructure` + 组装所需。

**Tests:** 每个被改 handler 的纯逻辑（解析/转换）抽成可测函数并补测。

**Commit:** `refactor(api): route handlers delegate to application services`（分批，每域一个 commit）

---

## Phase 6 — 清理旧 crate（消灭伪迁移）

### Task 6.1: 消费方切到 intelligence-domain

**Files:**
- Modify: `crates/intelligence/signal-engine/src/lib.rs` 及子模块
- Modify: `crates/claim-engine/src/lib.rs` 及子模块
- Modify: 所有 `use signal_engine::*` / `use claim_engine::*` 的调用方（api、worker-entry）

**Actions:**
- 把 signal/claim 的领域类型引用全部替换为 `intelligence_domain::*`（此时 Phase 2/3 已完成，端口可用）。
- 旧 crate 中仅保留纯逻辑部分（scoring/discovery/LLM 提取器）或直接删除——逐 crate 决定：无纯逻辑残留者整体删除。

**Tests:** 全局 grep `signal_engine::` / `claim_engine::` 归零；`cargo test --workspace` 全绿。

**Commit:** `refactor(signal-engine): migrate consumers to intelligence-domain` / `refactor(claim-engine): migrate consumers to intelligence-domain`

### Task 6.2: 删除 StoreBackend 与伪 domain 层

**Files:**
- Delete: `crates/store/src/backend.rs`（supertrait）
- Modify: `crates/store/src/lib.rs`、`crates/store/src/domain/mod.rs`
- Modify: `crates/store/src/memory/backend.rs`

**Actions:**
- 删除 `StoreBackend` supertrait 及全部遗留方法；`store::domain/*` 的 D1 CRUD 层按 Phase 3 搬迁情况清理，最终 `store` 只剩：D1 访问原语 + 通用 query 服务 + `traits/` 细粒度接口（供 infrastructure 使用）。
- 删除 `signal-engine`/`claim-engine`/`reflection-engine`/`memory-engine` 中已空的领域壳（或整个 crate）。

**Tests:** 编译零警告（`-D warnings`）；`cargo test --workspace` 全绿；架构测试（Phase 7）通过。

**Commit:** `refactor(store): delete StoreBackend supertrait` / `refactor: remove deprecated engine crates`

---

## Phase 7 — 架构护栏（防止复发）

### Task 7.1: 依赖方向自动化验证

**Files:**
- Create: `crates/shared-kernel/tests/architecture.rs`（或独立 `crates/architecture-guard`）
- Modify: `.github/workflows/lint.yml`

**Actions:**
- 用 `cargo metadata` 解析依赖图，断言：无循环依赖；`domain` 层 crate 不依赖 `worker`/`store`；`application` 不依赖 `worker`；`api` 不依赖 `infrastructure`/`vectorize`/`embedding`/`event-store`/`object-store`。
- 不引入新 crate（如 cargo-machete 等）——用 cargo-metadata JSON 即可，避免依赖膨胀。

**Tests:** CI 中作为独立 job 运行；任何回归导致红。

**Commit:** `test(governance): architecture dependency guard`

### Task 7.2: 基线更新

**Files:**
- Modify: `FULL_REVIEW_REPORT.md` §5（更新 Health Score / Tech Debt / 依赖图）
- Modify: `CLAUDE.md`（架构一节反映新依赖图）

**Commit:** `docs: update architecture baseline after decoupling`

---

## 完成定义（DoD）

1. `cargo test --workspace --all-features` 全绿，测试数不减反增（每域迁移必有测试护航）。
2. `cargo clippy --workspace -- -D warnings`、`cargo fmt --check` 通过。
3. `api` 的 `use store::` 引用归零；`application` 消费覆盖所有用例。
4. `intelligence_domain::` 为 signal/claim 领域类型的唯一来源；`signal-engine`/`claim-engine` 删除或仅存纯逻辑。
5. `StoreBackend` 删除；`store` 不再被 domain 层 crate 直接依赖。
6. 架构护栏测试通过并接入 CI。
