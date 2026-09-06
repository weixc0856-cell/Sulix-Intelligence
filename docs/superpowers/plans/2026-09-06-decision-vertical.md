# Decision Vertical Plan（2026-09-06 — 收掉 GATED 决策写 seam）

> 续 decoupling 主线（C1–C7，`application:store` 归零）。**目标**：让 decision 写路径真正走
> decision-engine 领域 aggregate，从而删除 GATED `DecisionWriteStore`（4 方法）与空 `StoreBackend`
> composite。**用户裁决（2026-09-06）**：立项做，先出本 plan，评审后逐 checkpoint 执行。
> **不变量**：outbox 事件契约原样、测试数不降、guard 空表、fmt/clippy/wasm 绿、每 checkpoint 独立 commit；
> **push = BLOCKED** 至用户确认。纯代码 + 一条 D1 migration 文件（不 apply 到 CF，不碰资源）。

---

## 1. 目标态（done 定义）

1. 生产 decision 写路径 = **use-case 编排 decision-engine aggregate**，经
   `decision_engine::DecisionRepository`（`D1DecisionRepository`，composition-root 构造）持久化；
   aggregate 状态（含 expected_outcomes，DAG 状态）可落库、可 hydrate。
2. 删除 GATED `domain::DecisionWriteStore` 4 方法 + 空 `StoreBackend` composite + 两个空 impl；
   worker-entry / infrastructure 不再引用 `StoreBackend`。
3. `application::services::decision::DecisionService`（生产死代码，DTO 直写 store）**消灭**；
   两个 `DecisionService` 撞名消解为唯一真身。
4. 读侧（DecisionReadService / graph / trust / reflection / context 的 `DecisionQueryService`/
   `domain::DecisionRepository` 读取）**行为不变**；decision_stats 桶语义按 SD-A 定案。
5. outbox 事件 type 字符串（`DecisionCreated`/`DecisionStatusChanged`/`OutcomeObserved`/
   `DecisionEvaluated`）、`object_type = "event:{agg}"`、先落库后发事件的顺序 —— **原样保留**。
6. 测试数 ≥ 379 且净增（mapping tests 保留/演进，新增序列化 + expected_outcomes round-trip + use-case）。

---

## 2. Recon 摘要（2026-09-06 agent 实测，file:line 略）

| 面 | 现状 |
|---|---|
| 领域 | `decision-engine` 有真 aggregate（`aggregate.rs`，仅 `Debug`，无 Serialize）；状态 DAG
  `Draft→Proposed→Approved→Executing→Completed\|Invalidated`（`status.rs`）；`ProposeDecision`
  **已带** `expected_outcomes`；aggregate 自带事件缓冲 + `drain_events`（`decision.proposed` 等）。 |
| 双份混乱 | 两个 `DecisionStatus`（新 DAG enum vs legacy `Active…`）、两个 `DecisionRepository`
  （`decision_engine::` 存 aggregate vs `domain::` 存 NewDecision/读）、两个 `DecisionService`
  （worker-entry 生产 vs application 死代码）并存。 |
| 写路径 | 生产唯一写 = worker-entry `DecisionService<S: StoreBackend>`：`create_decision`（INSERT
  status 硬编码 `'active'`）→ outbox `DecisionCreated` → `find_decision` 回读；`record_outcome` 发
  `DecisionStatusChanged{"status":"completed"}` 但**从不真翻行**。expected_outcomes 全程丢弃。 |
| 持久化 | D1 `decisions.status TEXT DEFAULT 'active'`；stats 桶 `active|completed|superseded`；
  `domain::DecisionRepository::save_decision(NewDecision)` 只 INSERT 无 upsert。 |
| outbox | `StoreBackend::insert_outbox`（= `OutboxStore` + D1 inherent），表 `object_outbox`；
  archive worker 按 `created_at ASC` drain，`event:*` → 事件归档。reflection/memory 也写同表（dual-seam）。 |
| infra | `D1DecisionRepository<S: StoreBackend>` 已实现 `decision_engine::DecisionRepository`
  （save/find/find_by_signal/list 于 aggregate），**8 个 mapping tests 就位但生产零接线**；
  已含 `status_to_d1`（Executing→'active'、Invalidated→'superseded'）与 hydrate。 |
| 读侧 | api read handlers 全走 `DecisionReadService`（`DecisionQueryService + domain::DecisionRepository
  + DecisionRecordStore + …`）；reflection/context adapter 读 `find_decision/list_*`。 |
| 架构守卫 | `GRANDFATHERED = []`；domain/application 不得依赖 composition/concrete-infra；application
  可依赖 `decision-engine`（domain 类）与 domain 端口。写路径只能住 composition-root / delivery 侧。 |
| 测试 | decision-engine 23 + infra 8 + application decision_read 6 + application dead-service 1 +
  graph 1 + trust 若干 + events/shared-kernel 12；store memory / worker-entry / api **无** decision 单测。 |

**recon 标出的核心风险**：切换写路径时 (a) outbox 事件 type/顺序必须零改动；(b) `find_decision`
回读语义保留；(c) `record_outcome` 假 "completed" 翻状态属半成品行为，SD-A 定案；(d) 删
`DecisionWriteStore` 前要先让真 save 路径接上（现有 8 个 infra mapping tests 已覆盖 aggregate→persist，
可承接死 service 测试的守护意图）。

---

## 3. 子决议（进 P1 前定案；默认值如标，若有异议先提）

- **SD-A status 词汇**：**默认 bounded 对齐、不动 D1 全量迁移** —— D1 继续存现有 3 个字符串桶：
  `Proposed/Approved/Executing → 'active'`（in-flight 归 active，与现状 stats 桶一致）、
  `Completed → 'completed'`、`Invalidated → 'superseded'`；`Draft` 不落库（propose 后即 Proposed）。
  DB 值域不扩列；`decision_stats` 桶语义不变。副作用：写完状态需真翻转（修 record_outcome 假 completed），
  行为从"永远 active"变为"终态落 completed/superseded"——stats 分布会如实变化，属**预期修正**。
  备选（更大）：全量迁新词汇 + stats 改桶 + migration CHECK，不做（除非用户要求）。
- **SD-B expected_outcomes 持久化**：**默认加一条 migration（0050）** `decisions` 加
  `expected_outcomes TEXT`（JSON 数组），insert/upsert 时写入、hydrate 时读回；`observed_outcomes`
  **不新增列** —— fact 层 `outcome_events` 已是持久记录，hydrate 时从 `outcome_events` 归并。
- **SD-C 编排归宿**：**默认** 真 use-case 放 `application::services::decision::DecisionService`
  （generic over `decision_engine::DecisionRepository` + `domain::OutboxStore`，二者皆 guard 允许的
  domain 端口）；worker-entry route handler 在 composition root 拿 `D1DecisionRepository`（infra，
  依赖 store）注入后调用。这样写编排上收 application、delivery 只做接线；`DecisionWriteStore` 随
  use-case 重写而删除。若执行中发现 use-case 泛化成本过高，可退回 worker-entry 直持 concrete repo
  编排（记 deviance，需说明）。

---

## 4. 目标态图（decision 写垂直）

```
delivery  worker-entry route ──(composition-root 构造)──> application::services::decision::DecisionService
                                                            │ generic over
                                                            ├── decision_engine::DecisionRepository   (aggregate save/find)
                                                            └── domain::OutboxStore                   (outbox 事件)
infrastructure  D1DecisionRepository implements decision_engine::DecisionRepository  (store D1 映射)
store           D1Store implements domain::{DecisionRepository, DecisionQueryService, OutboxStore, …读端口}（无 write vertical）
删：domain::DecisionWriteStore(4) + store::StoreBackend composite + 空 impl + application 旧死 DecisionService
保留不变：读侧（DecisionReadService/graph/trust/reflection/context）、api routes、outbox drain、archive worker
```

---

## 5. 执行 checkpoints（每个独立绿 + 独立 commit；逐个过）

### P0 — 定案 + recon 固化
- SD-A/SD-B/SD-C 拍板；本 plan 就是 recon 固化。
- **Gate**：doc 评审通过。

### P1 — decision-engine 域硬化（纯 domain，零 infra）
- `DecisionAggregate` 派生/实现 `Serialize + Deserialize`（status 用 serde snake_case；补 round-trip 测试）。
- repository 契约语义钉死：`save` = **upsert**（存在则更新、否则插入）；必要时在
  `decision_engine::DecisionRepository` 方法集上补签名或文档化 hydrate 契约（不加 speculative 方法）。
- 补 `expected_outcomes` hydrate/persist 的纯逻辑辅助（serde JSON 编解码放 domain，可单测）。
- **Gate**：decision-engine + status + memo 全绿；新增序列化/JSON 单测；guard 无新边。

### P2 — D1 持久化补齐（store + migration + infra adapter）
- migration `0050_decision_expected_outcomes.sql`：`ALTER TABLE decisions ADD COLUMN expected_outcomes TEXT`。
- store `domain::DecisionRepository::save_decision` 或 infra repo 的 SQL 层：支持带 expected_outcomes
  （JSON）的 insert，并把重复 save 变 **upsert**（`ON CONFLICT(id) DO UPDATE`，注意 `decisions.id`
  现为主键 → 用 `ON CONFLICT(id)` 需要 id 唯一约束，recon 确认 id PK）。
- `D1DecisionRepository::from_store`/`into_new` 补 expected_outcomes；`observed_outcomes` hydrate 从
  outcome_events 归并（首个 checkpoint 可先 only expected，observed 归并列 backlog 若成本高）。
- **Gate**：infra decision_repository mapping tests 更新 + 新增 expected_outcomes round-trip +
  upsert 二写不重复；store 测试全绿；migration 文件审阅。

### P3 — 真 use-case + 生产接线上收（核心切换）
- 重写 `application::services::decision::DecisionService` 为真 use-case：`propose(cmd)` →
  `DecisionAggregate::propose` → `repo.save` → `drain_events` → `outbox` 逐条发（**事件 type/object_type/
  顺序与现 worker-entry 完全一致**）；`change_status` 走 aggregate transition 后 save；`record_outcome`/
  `record_evaluation` 保留 create_outcome/create_evaluation 契约，按 SD-A 决定是否/如何翻状态。
- 删除旧 DTO 直写版 + 其单测的守护由 infra mapping tests（已存在）+ 新 use-case 测试承接（**不净减**）。
- worker-entry route handler 改构造/注入真 use-case（composition root 建 `D1DecisionRepository`）。
- **Gate**：`cargo test --workspace` ≥ 379 不降；clippy/fmt/wasm/guard 绿；两条既有生产 route 语义经
  infra round-trip 验证（先本地 memory 不可用 → 用 infra repo 映射测试 + store memory 测试覆盖）。

### P4 — 删 seam
- 删 `domain::DecisionWriteStore`（4 方法）+ store/memory 双 impl + 空 `StoreBackend` composite +
  两个 `impl StoreBackend for …{}` + `worker-entry`/`infrastructure` 里残余 `StoreBackend` 引用换窄端口。
- 删 `DecisionWriteStore` 不再被 guard/DTO/doc 引用后清理文档注释。
- **Gate**：`rg 'StoreBackend|DecisionWriteStore' crates --glob '*.rs'` 归零（文档注释一并清）；
  guard / layered / wasm / clippy / fmt / test 全绿。

### P5 — docs + 收尾
- CLAUDE.md、status-roadmap、decoupling-advance 补记 decision vertical 收口；ADR 若需则加。
- **Gate**：全绿 + docs 一致；push BLOCKED 等确认。

---

## 6. 风险与护栏

| 风险 | 护栏 |
|---|---|
| outbox 事件契约破坏（archive worker 下游断） | P3 事件 type/object_type/顺序**逐字节对齐**现 worker-entry；P4 前跑 `rg` 核对 4 个 event type |
| `find_decision` 回读语义变 | repo.save 必须返回 id 且能回读；P3 use-case 保留 create 后回读断言 |
| stats 分布因真翻状态而变（"预期修正"但可能被当回归） | SD-A 明示；P3 后跑 decision_stats 相关测试（trust/read）确认桶逻辑未坏 |
| 删 seam 时误伤读侧 | P4 前 infra 8 mapping tests 全绿为前置；P4 只删 write vertical 相关 |
| 测试净减 | 每 checkpoint 用 before/after 数核对 ≥379；P3 先加 use-case 测试再删死 service 测试 |
| 架构边 | use-case generic over domain 端口；repo 实例只在 composition-root/delivery；guard 每步跑 |
| schema 迁移扩列 | 仅 migration 文件（未 apply）；若 SD-B 被否，P2 改为 expected_outcomes 走现有表/放弃 |

## 7. Out of scope（明确不做）

- Decision read 路由重构 / DecisionRecordStore（Sprint 6.0 decision-records）不动。
- `domain::DecisionRepository`（读 DTO）与 `decision_engine::DecisionRepository`（aggregate）合并。
- intelligence-domain / P6（D1 已定：保留）。
- `crates/events` 与 `event_store::EventEnvelope` 两套信封合并。
- memory `DecisionRepository`（decision-engine repo）测试替身 —— application use-case 测试改用 infra
  mapping 事实 + 新增 domain 纯逻辑测试承接；若 P3 发现确需 memory repo 替身再单列（记 backlog）。

## 8. 关联

- recon：`docs/status-roadmap-2026-09-06.md`（D2 行）、decoupling-advance §5（GATED 由来）、`ADR-004`（D1）
- 决策/守卫：`final-architecture-v2.md` §4、`shared-kernel/tests/architecture.rs`
- 测试基限：`cargo test --workspace` = 379（2026-09-06）
