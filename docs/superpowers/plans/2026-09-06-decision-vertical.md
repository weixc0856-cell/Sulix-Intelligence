# Decision Vertical Plan（2026-09-06 — 收掉 GATED 决策写 seam）

> 续 decoupling 主线（C1–C7，`application:store` 归零）。**目标**：让 decision 写路径真正走
> decision-engine 领域 aggregate，从而删除 GATED `DecisionWriteStore`（4 方法）与空 `StoreBackend`
> composite。**用户裁决（2026-09-06）**：立项做，先出本 plan，评审后逐 checkpoint 执行。
> **评审（2026-09-06，有条件通过 → Ready for P1）**：架构方向 approved；已并入评审修订 —— SD-A1/A2
> 拆分（status 映射 ≠ outcome lifecycle）、SD-D（save/outbox 一致性边界，不引 UoW）、P1 aggregate
> invariant（禁 use-case/route 直写 status）、save 语义钉死（persist aggregate，D1 upsert 是实现细节）、
> P2 upsert 字段策略（created_at 保留）、删测试须有 replacement owner、P4 双 grep 终验防旧方法换名藏匿。
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
   `domain::DecisionRepository` 读取）**行为不变**；decision_stats 桶语义按 SD-A1 定案。
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
回读语义保留；(c) `record_outcome` 假 "completed" 翻状态属半成品行为，SD-A2 定案；(d) 删
`DecisionWriteStore` 前要先让真 save 路径接上（现有 8 个 infra mapping tests 已覆盖 aggregate→persist，
可承接死 service 测试的守护意图）。

---

## 3. 子决议（进 P1 前定案；默认值如标，若有异议先提）

> **评审拆法**：把"status"劈成两个正交问题 —— D1 怎么存（SD-A1）≠ record_outcome 是否完结 aggregate
> （SD-A2）；另立 SD-D 定 outbox 一致性边界。下面 SD-A1/A2/B/C/D 分列。

- **SD-A1 持久化 status 映射**：**默认 bounded 对齐、不动 D1 全量迁移** —— D1 继续存现有 3 个字符串桶：
  `Proposed/Approved/Executing → 'active'`（in-flight 归 active，与现状 stats 桶一致）、
  `Completed → 'completed'`、`Invalidated → 'superseded'`；`Draft` 不落库（propose 后即 Proposed）。
  DB 值域不扩列；`decision_stats` 桶语义不变。副作用：写完状态需真翻转，行为从"永远 active"变为
  "终态落 completed/superseded"——stats 分布如实变化，属**预期修正**。备选（更大）：全量迁新词汇 + stats
  改桶 + migration CHECK，不做（除非用户要求）。映射函数即现 `D1DecisionRepository::status_to_d1`。
- **SD-A2 outcome lifecycle（record_outcome 语义定案）**：recon 确认现 `record_outcome` 发
  `DecisionStatusChanged{"status":"completed"}` 事件但**从不真翻行** —— 这是半成品行为（bug/未完成），本次
  修正，不是纯架构清理。**默认**：若业务语义是"记录最终 outcome 即意味着 decision 完结"，则由 **aggregate
  transition** 真完成 `Executing → Completed`（发事件 + 转 status）；否则 outcome 只追加 fact、不改
  aggregate lifecycle。**裁决落在 aggregate 内，use-case/route 一律禁止 `status = Completed` 直写**（见 P1
  invariant）。P3 前用 route 源码核对 `POST /decisions/:id/outcomes`（record outcome）与
  `POST /decisions/:id/status`（显式翻状态）当前是否同一语义，再钉死 SD-A2 默认值。
- **SD-B expected/observed 持久化**：**默认加 migration（0050）** `decisions` 加 `expected_outcomes TEXT`
  （JSON 数组），insert/upsert 写入、hydrate 读回；`observed_outcomes` **不新增列**。两者来源不同，定为
  **不变式**（防止日后有人问"为何 expected 有列 observed 没有"）：

  ```
  expected_outcomes = aggregate state → persisted in decisions（serde JSON 数组；D1 字段映射由 repo 控制）
  observed_outcomes = observed facts  → reconstructed from outcome_events（事实层投影，hydrate 归并）
  ```

  `expected_outcomes` round-trip 是 P2 **必做**；`observed_outcomes` hydrate 若成本高可列 backlog，但
  **P2 不重设计 outcome_events schema**。
- **SD-C 编排归宿**：**默认** 真 use-case 放 `application::services::decision::DecisionService`（generic
  over `decision_engine::DecisionRepository` + `domain::OutboxStore`，二者皆 guard 允许的 domain 端口）。
  composition-root **同时构造 repo 实例 + outbox 端口** —— `D1DecisionRepository`（infra）+ 既有
  D1 outbox adapter / `D1Store as domain::OutboxStore` —— 一并注入 use-case。**use-case 只依赖窄端口，
  不依赖 `D1Store` 全貌**（P7 边界不绕回）。这样写编排上收 application、delivery 只做接线；
  `DecisionWriteStore` 随 use-case 重写而删除。若执行中发现 use-case 泛化成本过高，可退回 worker-entry
  直持 concrete repo 编排（记 deviance，需说明）。
- **SD-D save/outbox 一致性边界**：**默认不引入新 transaction/UoW 抽象、不扩 outbox 架构** —— 保持当前
  observable ordering："先落 decision，再写 outbox"（现 worker-entry 同序）。aggregate save 与 outbox
  append 之间若当前 D1 adapter 有可复用的一致性边界则复用，否则**如实记录为后续 reliability work**，
  **不在此 vertical 暗中宣称"aggregate saved → events atomically persisted"**。drain_events 时机不变量见 P3。

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
- SD-A1/SD-A2/SD-B/SD-C/SD-D 拍板；本 plan 就是 recon 固化。
- **Gate**：doc 评审通过。

### P1 — decision-engine 域硬化（纯 domain，零 infra）
- `DecisionAggregate` 派生/实现 `Serialize + Deserialize`（status 用 serde snake_case；补 round-trip 测试）。
  **serde 边界**：serde 只作 aggregate state 的内部序列化辅助，**不是 persistence contract** —— D1 字段
  映射仍由 `D1DecisionRepository` 显式控制；不把整个 aggregate JSON 塞进 D1（`expected_outcomes` 单列
  = JSON 数组已是正确方向）。
- **aggregate invariant**：所有 lifecycle mutation 必须经 aggregate 方法（`propose` / `approve` /
  `start_execution` / `complete` / `invalidate`，或现 DAG transition 的等价命名）；**禁止 use-case/route
  直写 status**。若现 aggregate 只有通用 transition API，P1 补具名方法或以文档钉死"禁直写 status"。
- **repository 契约语义钉死**：`save(aggregate)` = *persist current aggregate state by aggregate id*；
  D1 侧 `INSERT ... ON CONFLICT(id) DO UPDATE` 只是当前**存储机制的实现细节**（P2 实现），不是契约本身。
  必要时在 `decision_engine::DecisionRepository` 方法集补签名或文档化 hydrate 契约（不加 speculative 方法）。
- 补 `expected_outcomes` hydrate/persist 的纯逻辑辅助（serde JSON 编解码放 domain，可单测）。
- **Gate**：decision-engine + status + memo 全绿；新增序列化/JSON/lifecycle 单测；guard 无新边。

### P2 — D1 持久化补齐（store + migration + infra adapter）
- migration `0050_decision_expected_outcomes.sql`：`ALTER TABLE decisions ADD COLUMN expected_outcomes TEXT`。
- **save 实现（SD-B/SD-D 落地）**：`D1DecisionRepository::save` 落库 = `INSERT ... ON CONFLICT(id) DO UPDATE`
  （`decisions.id` 主键 → id 唯一，recon 确认）。**upsert 字段策略**：aggregate-owned 字段
  （title/description/signal_id/status/expected_outcomes，以现表列名为准）随 save 更新；**bookkeeping
  保留原值** —— `created_at` 不因二次 save 重写（首插落原值）、`updated_at` 刷新；不做整行盲覆盖。
- 支持带 expected_outcomes（JSON）的 insert；`D1DecisionRepository::from_store`/`into_new` 补
  `expected_outcomes` round-trip（P2 必做）；`observed_outcomes` hydrate 从 outcome_events 归并（SD-B
  不变式；首个 checkpoint 可先 only expected，observed 归并列 backlog 若成本高）。
- **Gate**：infra decision_repository mapping tests 更新 + 新增 expected_outcomes round-trip + upsert
  二写不重复 + created_at 保留断言；store 测试全绿；migration 文件审阅。

### P3 — 真 use-case + 生产接线上收（核心切换）
- 重写 `application::services::decision::DecisionService` 为真 use-case：`propose(cmd)` →
  `DecisionAggregate::propose` → `repo.save` → `drain_events` → `outbox` 逐条发（**事件 type/object_type/
  顺序与现 worker-entry 完全一致**）。
- **lifecycle 走 aggregate（不再有 `change_status` 旁路）**：`approve`/`start_execution`/`complete`/
  `invalidate`/`record_outcome`（SD-A2 语义）各调 aggregate transition 后 save；`record_evaluation` 保留
  create_evaluation 契约语义、按 SD-A2 决定是否翻状态。use-case 只组装命令 + 调 aggregate + 编排持久化。
- **outbox 顺序与 drain 不变量（SD-D）**：aggregate 事件是 **domain transition 的结果**（transition 时
  缓冲），不是 persist 成功的结果。use-case 顺序 = transition → (state changed, events buffered) → persist
  aggregate → persist events/outbox → 清缓冲；`drain_events` 一次即清空，**失败恢复语义在 use-case 测试里
  钉死**（save 失败则事件不半发、重试语义与现 worker-entry 一致）。
- **删除旧 DTO 直写版 + 其单测 → replacement 规则**：每个被删的 write-path behavioral test 必须有明确
  replacement owner（aggregate lifecycle 测试 / infra mapping tests（已存在）/ 新 use-case 测试）。
  计数 ≥379 是**硬下限**而非唯一 gate——允许"删低价值 + 高质量替换"。
- worker-entry route handler 改构造/注入真 use-case（composition root 建 `D1DecisionRepository` + outbox
  端口，见 SD-C）。**两条生产 route 语义经 infra round-trip 验证**：route → use-case → aggregate → repo →
  D1 → hydrate → 读侧 → API 响应。Create flow：request → Proposed → expected_outcomes persisted →
  DecisionCreated outbox → hydrate 回读。Outcome flow：record outcome → fact persisted → 按 SD-A2 的
  lifecycle → 正确 outbox 事件 → 读侧见预期 status。
- **Gate**：`cargo test --workspace` ≥ 379 不降；clippy/fmt/wasm/guard 绿；decision_stats 相关测试
  （trust/read）确认桶逻辑未坏。

### P4 — 删 seam
- 删 `domain::DecisionWriteStore`（4 方法）+ store/memory 双 impl + 空 `StoreBackend` composite +
  两个 `impl StoreBackend for …{}` + `worker-entry`/`infrastructure` 里残余 `StoreBackend` 引用换窄端口。
- 删 `DecisionWriteStore` 不再被 guard/DTO/doc 引用后清理文档注释。
- **Gate（双 grep 终验，防旧方法换名藏匿 / 改名留在 D1Store）**：
  ```bash
  rg 'StoreBackend|DecisionWriteStore' crates --glob '*.rs'        # 归零（文档注释一并清）
  rg 'create_decision|update_decision_status|create_outcome|create_evaluation' crates --glob '*.rs'
  ```
  第二 grep 结果**逐条人工分类**：旧 StoreBackend API 写方法 = 0；Decision aggregate / use-case 的 canonical
  方法（同名允许，属新 vertical）；`D1DecisionRepository` = canonical persistence。任何结果不得落在
  `StoreBackend` / `D1Store` 直写 API 上。guard / layered / wasm / clippy / fmt / test 全绿。

### P5 — docs + 收尾
- CLAUDE.md、status-roadmap、decoupling-advance 补记 decision vertical 收口；ADR 若需则加。
- **决策写路径唯一性收口（结构性规则）**：生产 decision 写 = worker-entry → application DecisionService →
  decision-engine aggregate → `decision_engine::DecisionRepository` + `domain::OutboxStore`。P4 删旧 API 后
  worker-entry 无路可触 `D1Store::create_decision()` 之类的旧写；`api → D1DecisionRepository` 已被 guard
  （api:❌concrete-infra）锁死。此规则写入 docs；若要作 architecture-test 断言，单列小 commit 再评估。
  read 侧 / `DecisionRecordStore` / 双 DecisionRepository 合并均不动。
- **Gate**：全绿 + docs 一致；push BLOCKED 等确认。

---

## 6. 风险与护栏

| 风险 | 护栏 |
|---|---|
| outbox 事件契约破坏（archive worker 下游断） | P3 事件 type/object_type/顺序**逐字节对齐**现 worker-entry；P4 前跑 `rg` 核对 4 个 event type |
| `find_decision` 回读语义变 | repo.save 必须返回 id 且能回读；P3 use-case 保留 create 后回读断言 |
| stats 分布因真翻状态而变（"预期修正"但可能被当回归） | SD-A1 明示；P3 后跑 decision_stats 相关测试（trust/read）确认桶逻辑未坏 |
| `record_outcome` 假 "completed" 半成品行为 | SD-A2 定案为真实 bug：P3 起由 aggregate 真翻转（或明确只记 fact），use-case 禁直写 status |
| save 与 outbox 被误宣称原子 | SD-D：不引 UoW；保持"先 decision 后 outbox"现 ordering；无可复用一致性边界则记 reliability backlog |
| 4 个旧写方法换名藏进 D1Store / use-case 直写 status | P1 aggregate invariant + P4 双 grep 终验人工分类 |
| 删 seam 时误伤读侧 | P4 前 infra 8 mapping tests 全绿为前置；P4 只删 write vertical 相关 |
| 测试"数量够但质劣 / 净减无主" | ≥379 为硬下限；replacement 规则：每个被删 write-path behavioral test 有明确 owner（P3）；每 checkpoint before/after 核对 |
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
