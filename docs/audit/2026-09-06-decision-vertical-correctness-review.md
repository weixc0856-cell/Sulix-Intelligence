# Decision Vertical — Correctness 逐条核验报告（2026-09-06）

> **触发**：全项目 review（2026-09-06）提出 decision vertical 四个 P0 隐患：① hydrate 空 `observed_outcomes`、② `MAX(id)+1` 并发、③ `evt_{ts}_{seq}` event_id 撞车、④ outbox best-effort 丢事件。
>
> **本次动作**：不写结论，把四条断言逐条对着 HEAD（17bd9ac）真实代码核验，并修正触发面 / 严重度 / 修复成本。
>
> **范围**：`decision-engine` aggregate、`application::DecisionService`、`infrastructure::D1DecisionRepository`、`store/d1/decision`、`worker-entry decision_write`、`event-store` keys、domain 决策端口。**不含**两套 domain 命名、orphan crate、signal-engine 定位（那些按 review 自评属 P2/P3，不在此核验）。
>
> **方法**：全部结论有 `file:line` 出处；未逐行核验处明确标注「待验证」，不猜测。

## 0. 裁决总表

| # | 断言 | 裁决 | 修正后的严重度 | 一句话 |
|---|------|------|---------------|--------|
| 1 | hydrate 空 `observed_outcomes`，aggregate 非完整状态 | ✅ **成立** | **中高（真缺陷，但 hot path 自洽）** | 现 route 流程恰好在同一内存 aggregate 上 attach→complete，绕过去了；踩空的是 `/status` complete、事件流 `outcome_count`、下游读 |
| 2 | `MAX(id)+1` 并发可发重号丢行 | ✅ **成立** | **中（概率低、修法结构性）** | 竞态真实；但 review 方案 B（DB 自增）需翻转 create 流程，本轮不值得 |
| 3 | `evt_{ts}_{seq}` event_id 不全局唯一 | ✅ **成立（且是必现，非"可能"）** | **中** | 每次 outcome-completing 请求两个 envelope 用同一 `t`+`seq`，event_id 必重复 |
| 4 | outbox best-effort 会静默丢事件 | ✅ **成立（但关键点在别处）** | **中** | 真风险是**与兄弟垂直不一致**：reflection/memory/event-store 都 fail-closed，只有 decision route 吞错 |

**总判断**：四条全部成立，review 没有空穴来风。但按触发面重排，#1 才是唯一值得本轮改代码的（含被测试替身掩盖、需 migration 级小决策）；#3 修复最便宜、必现、应顺手收；#2 #4 是**架构分叉**，正确动作是 ADR 固化取舍 + 恢复手段，不是本轮重构。

---

## 1. 断言 ① —— hydrate 空 `observed_outcomes`（成立，但触发面比 review 窄）

### 证据

- [decision_repository.rs:72-74](../../crates/infrastructure/src/decision_repository.rs#L72-L74)：`observed_outcomes: vec![]`，注释「hydration from events is a P2 backlog item」。confirmed。
- [into_row:83-99](../../crates/infrastructure/src/decision_repository.rs#L83-L99)：`decisions` 行**没有** observed_outcomes 列，`into_row` 只映射 row 字段 → aggregate 里的 `observed_outcomes` 在落库时必然丢弃。
- [aggregate.rs:95-113](../../crates/decision-engine/src/aggregate.rs#L95-L113)：`reconstruct` 原样接受外部喂的 `observed_outcomes`，责任全在 repository。

### 为什么 hot path 没炸（review 说对了但没说透）

[decision.rs:200-226](../../crates/application/src/services/decision.rs#L200-L226)：

```
load()            → hydrate（observed_outcomes = []，status = Executing）
was_executing     → true
save_outcome()    → outcome_events 事实行落库
attach_outcome()  → 内存里 push 本次这一个 outcome
complete()        → len == 1，通过 invariant
repo.save()       → 行翻到 completed
```

`record_outcome` 是 **load → attach(本次) → complete() 全在同一内存 aggregate** 上完成，所以当前唯一写路径自洽。review 的「恰好绕过」判断准确。

### 真正踩空的触发面（这才是要修的）

1. **`/status` 收 `"completed"`**（[decision.rs:181](../../crates/application/src/services/decision.rs#L181) 调 `decision.complete()`）：一个 `Executing` 决策若已有 outcome_events（例如 outcome 在其非 Executing 期录过、或 legacy 数据），hydrate 后 `observed_outcomes=[]` → `complete()` 抛 `MissingOutcome`，**尽管 DB 里 outcomes 存在** → 400「decision has no observed outcome」。这是 Persistence Model 与 Domain Model 不一致的直接命中。
2. **`Completed` 事件 `outcome_count` 恒 = 1**（review 漏掉）：[aggregate.rs:283-287](../../crates/decision-engine/src/aggregate.rs#L283-L287) 的 count 取**内存** `observed_outcomes.len()`。hydrate 为空 → 完成事件的 count 永远是 1，即使该决策累计了多个 outcome_events。事件流数据质量错。
3. **测试替身掩盖了缺陷**（review 漏掉）：use-case 测试的 `MemAggregateRepo` 走 **serde 全量快照**（[decision.rs:307-323](../../crates/application/src/services/decision.rs#L307-L323)），会保留并回读 `observed_outcomes`；而生产 `D1DecisionRepository` 走 row→reconstruct、丢 observed。**两种 repository 语义不一致** → 397 个测试全绿但 bug 在 D1 路径存活。

### 修复成本核验

- 读端口**已存在**：[decision_query.rs:24](../../crates/domain/src/traits/query/decision_query.rs#L24) `list_outcomes(decision_id) -> Vec<OutcomeEvent>` 已在 `DecisionQueryService` 内，而 `D1DecisionRepository` 的 bound **已含** `DecisionQueryService`（[decision_repository.rs:23](../../crates/infrastructure/src/decision_repository.rs#L23)）→ **不需要新增 seam**。
- 唯一结构性障碍（review 漏掉）：`ObservedOutcome` 带 verdict 枚举（[aggregate.rs 字段](../../crates/decision-engine/src/aggregate.rs)），而 `outcome_events` 事实行没有对应列 —— 应用层映射已硬编码 `OutcomeVerdict::Inconclusive`（[decision.rs:263-271](../../crates/application/src/services/decision.rs#L263-L271)）。**忠实重建 verdict 需要 migration 加列；接受降级则可纯读侧重建。** 这是需要 owner 拍板的小语义决定。
- 工程量：`from_store` 目前是同步纯函数且被 `find / find_by_signal / list` 复用；hydrate 需 async + 每行 N+1 读（或 join）。**可行但非一行改动。**

---

## 2. 断言 ② —— `MAX(id)+1` 并发重号（成立，概率低，修法是结构性的）

### 证据

- [crud.rs:11-22](../../crates/store/src/d1/decision/crud.rs#L11-L22)：`SELECT COALESCE(MAX(id), 0) + 1 AS next FROM decisions`。confirmed。
- [decision_id_source.rs:14-16](../../crates/domain/src/traits/decision_id_source.rs#L14-L16)：single-writer 假设已写进 domain trait 文档，标注「documented risk」。**这已不是未决问题，而是已文档化取舍。**
- 竞态后果链：[application/decision.rs:131-137](../../crates/application/src/services/decision.rs#L131-L137) 先分配 id 再 `propose`（`DEC-{id}` 在 propose 事件里就被嵌入）；两个并发 create 同算 101 → 第二个 `ON CONFLICT(id) DO UPDATE` 会**覆盖**第一个的整行 → 决策 A 静默丢失。

### 修正 review 的方案 B 成本

「让 DB `INTEGER PRIMARY KEY AUTOINCREMENT`，aggregate 再拼 `DEC-{rowid}`」**不是局部修**：decision id 的数字后缀**就是** `decisions` 行主键（[decision.rs:245-247](../../crates/application/src/services/decision.rs#L245-L247) `DEC-{id:06}`、[crud.rs 注释](../../crates/store/src/d1/decision/crud.rs#L11-L14) 明言「row primary key is written explicitly from the aggregate id」），且 propose 事件在 save 前就含 `DEC-{id}`。要 DB 出 id 就得把「先插行→拿 rowid→再 propose」翻过来，并把 aggregate 的 id 格式与 row pk 解耦 —— **结构性改动 + migration + 事件契约变更**。

### 建议

产品是 solo + cron 驱动为主的写入口，HTTP create 并发真实但低频。现实动作：
- **保留 single-writer，升级为正式 ADR**（含风险表述），不再停留在 trait 注释；
- create 上加轻量冲突探测（落库后读回校验 / 撞号即 409+重试），堵住静默覆盖；
- 把「DB 自增 + id 解耦」作为**已记录的后续重构项**，不在 correctness 轮做。

---

## 3. 断言 ③ —— event_id 不全局唯一（成立，且必现）

### 证据

- [event-store/lib.rs:205-208](../../crates/event-store/src/lib.rs#L205-L208)：`format_id(created_at, seq) = evt_{created_at}_{seq}`。confirmed。
- 撞车点：[decision_write.rs:300-331](../../crates/worker-entry/src/routes/decision_write.rs#L300-L331)。一次 outcome-completing 请求**同一个 `t`**（只取一次 `now()`），**同一个 `seq = outcome_id`**，先后发两个 envelope：

| envelope | aggregate_type | event_id |
|---|---|---|
| `OutcomeObserved` | outcome | `evt_{t}_{outcome_id}` |
| `DecisionStatusChanged` | decision | `evt_{t}_{outcome_id}` ← **相同** |

`seq` 语义其实是「decision_id 或 outcome_id」，不是 aggregate 内 sequence（create [L202](../../crates/worker-entry/src/routes/decision_write.rs#L202)、status [L264](../../crates/worker-entry/src/routes/decision_write.rs#L264)、evaluation [L375](../../crates/worker-entry/src/routes/decision_write.rs#L375) 全是 id）。review 说「可能得到相同 id」**低估了**：outcome-completing 每次请求**必然**产出两个同 id 事件。

### 现状是否已经激活（部分未核验）

- R2 对象 key 含 aggregate_type（[event-store/lib.rs:200-202](../../crates/event-store/src/lib.rs#L200-L202) `{aggregate_type}/{date}/{event_id}.json`）→ 两事件 R2 key 不同，**不互相覆盖**。
- `event_archive_index` / outbox 是否有 `event_id` UNIQUE 约束、重复插入会如何 —— **待验证**（见 §5）。若 index 以 event_id 为主键，completing 请求第二个 insert 会冲突/覆盖，需实测确认。

### 修复（便宜）

seq 换成每次 emit 单调递增 token（或直接用 drained event index / 请求内计数器）。不涉及 schema。

---

## 4. 断言 ④ —— outbox best-effort 静默丢事件（成立，但关键点被 review 带偏）

### 证据

- [decision_write.rs:46-59](../../crates/worker-entry/src/routes/decision_write.rs#L46-L59)：`let _ = insert_outbox(...).await`。confirmed。
- 消费链真实存在：[runtime/cron.rs:57-58](../../crates/worker-entry/src/runtime/cron.rs#L57-L58) 定时跑 [jobs/archive.rs:17-71](../../crates/worker-entry/src/jobs/archive.rs#L17-L71) drain outbox → R2 archive + event index；insert 失败则**根本没有 outbox 行** → archive worker 见不到 → **事件永久缺席，无恢复路径**。（insert 成功后的处理失败有 `mark_outbox_failed` 重试，但那救不了 insert 失败。）

### 关键修正：这不是「设计取舍」，是「内部不一致」

review §十一 承认 best-effort 但视为一体设计。实际上**同仓库其他事件源全部 fail-closed**：

- [reflection_repository.rs:104](../../crates/infrastructure/src/reflection_repository.rs#L104) `insert_outbox(...).map_err(...)` → 传播
- [memory_repository.rs:53](../../crates/infrastructure/src/memory_repository.rs#L53) → 传播
- [event-store/r2_backend.rs:31-34](../../crates/event-store/src/r2_backend.rs#L31-L34) outbox-first，错误传播

只有 decision route 吞错。所以「best-effort by design (SD-D)」中的「design」不成立 —— 这是 **decision 单点与体系策略相悖**。要么承认 decision 是唯一可容忍丢事件的源（需 ADR 说明为何可容忍），要么与体系对齐。

### 结构约束（review 自己承认的不可兼得）

SD-C 让 delivery 在 use-case 成功**之后**才拼装 envelope → 不可能和决策行同一事务写 outbox。D1 无跨表事务。真选择只有：
- **接受 + reconciliation**：envelope 全部可由 decisions / outcome_events / decision_evaluations 行**重放**（每类事件有确定性的行来源），补一个定时/按需对账即可闭环。与 fail-closed 兄弟垂直不冲突（它们丢的是行写之后的追加失败，这里是行写成功但事件构造失败——语义不同）。
- **推翻 SD-C**：envelope 进 use-case 事务（同 batch 写）—— 大改，违背刚收口的 D2 分层。
- **重试**：insert 失败 return 500 让客户端重放—— 与现有幂等（已到目标 status 即 no-op）兼容性需验证。

### 建议

ADR 固化「decision 事件 = 可从事实行重放」+ 记录 reconciliation 为后续项。**不本轮改代码。**

---

## 5. 待验证项（动手前必须确认）

1. `event_archive_index` schema：`event_id` 是否 UNIQUE？断言 ③ 的重复插入在此是否已冲突？（查 `migrations/`）
2. 是否有任何下游消费方把 event_id 当去重/幂等键（archive 之后谁读事件流）？
3. `outcome_events` 现有列集合 vs `ObservedOutcome` 字段 —— 决定 hydrate 走「加列」还是「verdict 降级」（断言 ① 的 migration 级决策）。
4. `reflection-engine` / `agent-engine` 是否经 `DecisionRepository` 加载 aggregate 并依赖 `observed_outcomes()` —— 决定断言 ① 的优先级口径。

---

## 6. 建议的处置（按改动风险升序）

| 序 | 动作 | 性质 | 触发面 | 改动 |
|---|---|---|---|---|
| 1 | event_id 唯一化（每 emit 单调 token） | 修 bug | 断言 ③ 必现 | 局部，route 层 |
| 2 | hydrate `observed_outcomes` from `outcome_events` | 修 bug | 断言 ①（含测试替身语义对齐） | decision_repository + 补 `complete-after-prior-outcome` 回归测试；需先定 verdict 策略 |
| 3 | create 撞号 409+重试（或读回校验） | 加固 | 断言 ② | 局部 |
| 4 | ADR：decision 事件可重放 + reconciliation；decision 单点 best-effort 与体系对齐说明 | 文档化取舍 | 断言 ④ | docs |

**不在本轮**：DB 自增 + DEC 解耦、outbox 事务化（推翻 SD-C）。两者按各自 ADR 记录为后续重构项。

---

## 7. 对全项目 review 的总体回应

架构方向判断（架构 8.5 / 领域闭环 6 / 该做 correctness）**成立**；四条 P0 断言**无一条落空**。但按本次核验，真正需要在 correctness 轮动代码的是 **断言 ③ + ①**，②④ 的优先级应让位于「先让 aggregate 成为事实上的完整 source of truth、事件 id 可唯一标识」这两个领域语义收口 —— 与 review 自己 §XIV「不要机械迁移、先定语义」的告诫自洽。

---

## Resolution Addendum（2026-09-06）— §5 核验结论 + ③① 已落代码

§5 四项待验证项查清的结论，以及后续落的两处修复（**已 commit**：代码 `425ea53`、本文档 `3cd1b99`，2026-09-06；未 push）：

**§5 核验结论**

1. **event_archive_index `event_id UNIQUE`**（`migrations/0022`:16）+ archive 插入 `INSERT OR IGNORE`（`store/d1/event_archive.rs`）→ **③ 是激活缺陷**：outcome-completing 每次请求，`DecisionStatusChanged`（与 `OutcomeObserved` 同秒同 `outcome_id`）被 OR IGNORE 静默挤出 → 完成事件从可查询事件史永久缺失（R2 payload 仍在）。非"可能"，是必现。
2. **下游依赖**：event_archive_index 即以 event_id 为行键（读侧 `store/d1/event_archive.rs` + infra event-log）→ 撞车直接造成事件史缺行。`format_id(sec, seq)` 同模式还被 `infrastructure/event_log.rs`、`signal_event_log.rs` 使用，但同请求双发同 id 的确定性缺陷只在 decision_write。
3. **outcome_events 列集**：`outcome_type, observation, result, accuracy, evidence_url, observed_at, created_at`；现代写路径只插前四类字段（`store/d1/decision/outcome.rs`），**无任何列存 verdict，写路径本就没产出 verdict** → 「加 verdict 列」无源可写。**结论：hydrate 降级为 `Inconclusive`**（与写路径映射一致），verdict 忠实化需新列 + 调用方输入（记 SD-B backlog）。
4. **下游 aggregate 消费者**：全仓只有 `application::DecisionService` + `D1DecisionRepository` 用 `DecisionRepository`，无任何 engine 经 repo 加载 aggregate → ① 影响面限 decision vertical 自身 + 事件数据质量。

**③ 修复（decision_write）**：envelope event_id 由 `evt_{sec}_{seq}`（seq=行 id）改为 `evt_{sec}_{ms}_{seq}_{nonce}` —— 与遗留 `evt_{sec}_{seq}` 四段 vs 两段**构造性不撞**，同秒跨请求由 ms+nonce 消歧。R2 key 仍按秒粒度 `occurred_at` 分区。

**① 修复（D1DecisionRepository）**：`find / find_by_signal / list` 从 `outcome_events` fact 行 hydrate `observed_outcomes`（复用既有 `DecisionQueryService::list_outcomes`，无新 seam；`MemoryStore` 已实现）。此前空 hydrate 会让「Executing + 已有 outcome_events」的决策在 `/status completed` 抛 `MissingOutcome` 且 `Completed` 事件 `outcome_count` 恒=1。verdict 按 §5.3 降级。附回归测试 `find_hydrates_observed_outcomes_from_outcome_events`（含 MemAggregateRepo/D1 语义差异被掩盖的场景）。

**验证**：`cargo test --workspace` **398 passed**（基线 397 + 新测试）；clippy（infra + worker-entry，`-D warnings`）干净；`cargo fmt --check` 干净；wasm gate（`cargo check -p worker-entry --target wasm32-unknown-unknown`）通过。变更：`crates/worker-entry/src/routes/decision_write.rs`、`crates/infrastructure/src/decision_repository.rs`。

### 其余 `format_id(sec, seq)` emitter 撞车面排查（2026-09-06，结论：无激活面，不改码）

逐一核验全仓每个 event_id 发射源（对 commit `425ea53` 修复后的 HEAD）：

| emitter | event_id 构造 | seq 语义 | 撞车判定 |
|---|---|---|---|
| `worker-entry decision_write`（outcome 双发） | 已改四段 `evt_{sec}_{ms}_{seq}_{nonce}` | 每 emit 随机消歧 | **已修**（③，`425ea53`） |
| `infrastructure/event_log.rs` `EventStoreLog`（reflection） | `evt_{occurred_at}_{seq_for_aggregate}` | seq = aggregate_id 的确定性 64-bit 哈希（FNV），**同 aggregate 恒定** | **无激活面**：reflection-engine 每次生成只 append 一个 `DomainEvent`，aggregate_id = 每轮唯一的 `REF-{reflection_id}`（`service.rs:212-220`）→ 同秒同 aggregate 双发不可达。哈希的**确定性是 retry-dedup 的 load-bearing 特性**（注释明言 re-append 同 id），改成随机会破坏重试幂等 —— **保留不动** |
| `infrastructure/signal_event_log.rs` `EventStoreSignalLog`（signal） | `evt_{occurred_at}_{sequence}` | sequence = 引擎 run 内单调 `events_written`（每次 append 前 +1，`signal-engine/lib.rs:136-155`） | **无激活面**：run 内同 `now` 但 seq 单调不等；跨 run 30-min 间隔，同 wall-second 不可能。仅「两 run 并发且内部计数同值落同秒」才理论撞 —— 当前 signal job 单 cron、不并发 |
| `event-store` 写路径（`event_archive_index`） | 透传 envelope.event_id | — | `UNIQUE(event_id)` + `INSERT OR IGNORE` 是**放大器**：任一上游重复 id → 第二个事件从索引史静默消失。上游全部排查后仅 decision_write 曾触达（已修） |
| `shared-kernel/events.rs` `event_id()` | `evt_{fastrand::u64}` | 随机 64-bit | 概率性，可忽略 |

**结论**：§5.2「确定性撞车只在 decision_write」**证实**。两类 infra adapter 的 seq 空间（run 内单调 / 逐事件新鲜 REF id）使同秒同 id 不可达；reflection 的确定性哈希应保留。**无新增代码改动。**

### ADR-005 / ADR-006 固化（② ④，2026-09-06）

按 audit §2/§4 建议把两个架构分叉升为正式 ADR：
- **ADR-005** `docs/decisions/005-decision-id-allocation.md` —— ② 保留 single-writer `MAX(id)+1`；DB 自增 + `DEC`/row-pk 解耦记为后续重构项。
- **ADR-006** `docs/decisions/006-decision-event-outbox.md` —— ④ decision outbox best-effort 收口为「decision 专属策略（事件可从事实行重建，因此可容忍）+ reconciliation 后续项」；不推翻 SD-C/SD-D，不把 best-effort 扩散到 fail-closed 的兄弟源。

**仍未动**（各自 backlog / 需 owner）：create 撞号加固的代码落地、outbox reconciliation job、verdict 列 + 写路径 verdict 源、`event_log.rs` 同 aggregate 同秒多类型事件的（当前不可达的）结构性护栏。
