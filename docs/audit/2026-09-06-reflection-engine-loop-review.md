# Reflection Engine / 决策学习闭环现状核验（2026-09-06）

> **触发**：Signal Engine 核验封板后（用户拍板 Signal 上游 A/B 均挂起、只修读侧时间线分叉），新主线按业务
> 闭环顺序推进 **Reflection Engine 核验（第二优先）→ Agent → 跨 vertical 集成**。本报告严格沿现有代码走，
> **不重新设计 Reflection、不改任何代码**（含不顺手改 Signal），只核验并分类。用户要求重点查 5 件事：
> ① 真实输入（是否真被 Decision→Outcome 数据喂给）
> ② 真实领域 lifecycle（有无 aggregate / 状态机）
> ③ 持久化完整性 + hydrate 一致性（snapshot/DB 错位、事件/事实丢失、ID/唯一性、outbox、测试替身掩盖）
> ④ Reflection→Memory 是否真实接通还是 port-only
> ⑤ 最终只分类 **P0 真实 correctness bug / P1 真实 lifecycle·生产 gap / P2 架构债务 / P3 暂不处理**
>
> 核验方法：一手通读 engine/service/context/repository + jobs·routes·cron·http·wrangler + store
> reflection crud + migrations/0024；三路子代理交叉取证（A: 反射读侧与产物消费者；B: Reflection→Memory
> 边与 memory-engine；C: artifact/event/archive 持久化完整性）。每结论挂 file:line，代理侧结论标注
> 「代理 A/B/C」。**未跑生产 D1 行数 / 未连生产 LLM** ——「生产不跑」「不可达」由代码静态推理。

## TL;DR

Reflection 是决策垂直（D2，已封板）之后的**第二段闭环反馈节点，但目前是一台「生产未点火、点一次火
必失败」的机器**：

- **意图**（lib.rs:1-4 + design spec）：Decision + Thesis + Evidence + Outcome → **Lessons + Decision Rules**，
  是「决策学习循环」的 feedback node。
- **真实存在的**：引擎本体（service.rs 管线完整、有单测）、`reflections` D1 行、R2 artifact 写入、EventStore
  事件；decision→outcome 输入**数据真实**（读 `outcome_events` 事实层，非空壳）。
- **生产状态**：reflection cron **关**（wrangler.toml 注释掉 CRON_REFLECTION_ENABLED，默认 false）、memory cron
  **关**；唯一活路径 = 手工 `POST /decisions/:id/reflect`（且需 AI_API_KEY 否则 Noop 生成必败）。
- **断点（按严重度）**：
  1. **失败一次即永久死锁**：`UNIQUE(decision_id)` + 重试永远重新 `INSERT` + `mark_failed` 拿错 id 查
     retry_count → 恒 0 → `<3` 上限永不触发 → 决策一旦生成失败就永远无法再反射，cron（一旦开启）每轮
     对同一 decision 撞唯一约束空转（R-1，P1）。
  2. **引擎产物无任何读路径**：lessons[]/rules[] 只在 R2 artifact（write-only，read()/find_by_owner 零调用且
     源码注释自认 broken），HTTP 无 GET 路由，决策 timeline 只取 `result` 摘要字符串（R-2，P1）。
  3. **Reflection→Memory 是 port-only 且查询错**：`find_event_keys("reflection", "", …)` 传空 aggregate_id
     恒零行，候选字段全硬编码 0/0/""（R-4，P1）。
  4. step6 artifact 失败即吞 + 伪造 artifact_key 永不落盘 → 内容静默丢失（R-3，P2）；step8 legacy outbox
     双写 = 不可归档毒行 + 共享 drain guard 反写（R-5，P2）；completeness gate 死代码（R-6，P2）；无
     aggregate/无状态机（R-7，P2）。

一句话：**输入段真实（决策/outcome 都是真数据），引擎管线完整，但产出段（lessons/rules）既不可读也不
通往 memory，且失败路径会把决策永久锁死 —— 决策学习闭环目前没有闭合。**

## 1. 生产触发面（先定坐标）

| 触发 | 状态 | 证据 |
|---|---|---|
| Cron `process_pending_reflections` | **关**（feature-flag 默认 false，wrangler 未设） | `runtime/cron.rs:61-63` `if cfg.reflection_enabled`；`wrangler.toml:23` 注释「phased in after signal stable」 |
| HTTP `POST /api/intelligence/decisions/:id/reflect` | **开**（唯一活路径，需 AI_API_KEY） | `runtime/http.rs:53` → `routes/reflection.rs:111 reflect` |
| GET 反射读路由 | **不存在** | http.rs 全表无 `/reflections` GET（代理 A） |
| Memory cron | **关** | `runtime/cron.rs:66-68`；wrangler.toml 未设 CRON_MEMORY_ENABLED（代理 B） |

## 2. 五问答案

### ① 真实输入 —— 决策侧真，上下文侧半空，且 API 路径无资格校验

- **决策/outcome 数据真实**：`D1ReflectionRepository::load_decision_context`（infrastructure/reflection_repository.rs:65-91）
  读 `find_decision` + `list_outcomes` + `list_evaluations`。`OutcomeQueryService::list_outcomes` 直接读
  `outcome_events` 事实层（d1_delegate.rs:388-392 → d1/decision/outcome.rs:41-56），outcome 由 decision
  vertical 的 save 路径写入 —— operator 记的 outcome 是真数据，含 observation 文本。
- **但富化上下文是占位**：`context.rs:90-94` `assumptions: Vec::new()`；`evidence: Vec::new()`（:108）；评分里
  `evidence_score = 0.2` 是**写死的占位**（:100 注释「could check signal evidence」）—— 反射 prompt 永远只看
  到 decision+hypothesis+outcome（+可选 evaluations），**看不到任何 signal/evidence/assumptions**。Signal
  blind（与 Signal 核验报告 §1.2 Reflection 行一致，context.rs:100 占位即指此处）。
- **Cron 资格查询要求真状态**：`decisions_eligible_for_reflection`（store/d1/reflection/crud.rs:106-125）要求
  status ∈ (completed, superseded) 且 >7d。但 **API reflect 路由不做任何资格校验**（routes/reflection.rs:111-135：
  只解析 id → 直接 execute）—— 可对任意 decision（包括无 outcome、Executing 中）触发反射。

### ② 真实领域 lifecycle —— 无 aggregate / 无状态机，与 Signal 同量级

- `ReflectionEngine`（service.rs:52-63）是 generic service 编排器：`execute_at`（:93-223）＝ create 行 → lease
  update → context → completeness → LLM → validate → 4 个持久化 sink。无 `ReflectionAggregate`、无命令方法、
  无状态转移守卫。状态只存在于 `reflections.status` 行，靠 `repository.update` 局部 patch 推进。
- 状态机本应有的语义靠 **SQL 排程 + 行 update** 表达（jobs/reflection.rs + crud.rs 三个排程查询），与
  Signal 的「行模型 + DB 维护任务」同构；与 decision-engine 的 aggregate 语义（D2）是两个量级。
- `UNIQUE(decision_id)`（0024:24）让一个 decision 至多一行 —— 强于 Signal，但**把「重试」逼成了不可能**
  （见 R-1）。

### ③ 持久化完整性 + hydrate 一致性 —— 行是真源，产物在 R2 不可读；事件不可回放重建

- **hydrate 方向与 D2 相反**：reflections **行**是真源；事件（ReflectionGenerated）是 append-only 通知，
  **没有任何 hydrate-from-events 路径**。若行丢失，事件无法重建反射内容（无 snapshot 事件、无事件承载完整
  lessons/rules）。无 D2-① 那种「find 从旧列读」的 snapshot/DB 错位 —— 该类在本域 **N/A**，但镜像缺口：
  「完整产物只在 R2，行只留摘要」。
- **事实丢失面**：① step6 artifact store 失败即吞 → 伪造 `memory/reflections/REF-*.json` 键永无人写 → 完整
  内容丢失，只余 `result` 摘要 + 计数（R-3）；② step8 outbox 行不可归档 → 毒行（R-5）；③ lessons/rules
  写完无任何消费者（R-2）。
- **ID/唯一性**：重试轮次 event_id **不撞** —— 每轮新 REF-id → seq hash 不同 → event_id 不同（代理 C；
  event_log.rs:31-47 seq_for_aggregate(aggregate_id)）。D2-③ 类在本域干净。但 `job_id = job_reflect_DEC{decision}_{now}`
  + `UNIQUE(decision_id)` 使**同一 decision 第二次 execute（含重试）在 create 就失败**（R-1）。
- **outbox**：step8 双写 sink 是 legacy 遗留，payload 非 `EventEnvelope`（R-5）。
- **测试替身掩盖（同 D2-① 模式，两个实例）**：
  1. service.rs 单测 `FakeRepo::find_latest_for_decision` 对任意入参都返回 `Some(record{ decision_id: 入参, retry_count: 0 })`
     （service.rs:320-326）→ `mark_failed` 的「拿 reflection_id 当 decision_id 查」错误在 D1 适配器上必然
     None→0，但 fake 永远 Some→恒 0，**单测绿、D1 语义错**（R-1 根因之一）。
  2. `step8_outbox_and_step9_event_log_dual_write`（service.rs:430-471）**钉死**了「两个 sink 各写一次、payload
     逐字节一致」为正确行为 —— 而 step8 sink 实际是排程器无法归档的 junk（R-5）：测试把缺陷当规范固化。

### ④ Reflection→Memory —— port-only，且断在一行空 aggregate_id 上

**结论：not operational。** 管道（类型/端口/适配器/事件通道）都在，但生产路径被一个查询 bug 打穿 + 字段
全硬编码 + 双 cron 关：

- memory cron → `D1MemoryRepository.list_reflection_events`（infrastructure/memory_repository.rs:56-67）→
  `store.find_event_keys("reflection", "", limit)`（**:57，字面空 aggregate_id**）；而反射事件索引在
  `aggregate_id = "REF-…"`（service.rs:215）。`find_event_keys` 两后端都是**严格等值**（store/d1/event_archive.rs:45-66
  `WHERE aggregate_type=?1 AND aggregate_id=?2`；memory/backend.rs:959 同）→ **零行**（代理 B）。
- 即便有行：`candidate.rs:26-34` 硬编码 `decision_id:""`/`quality_score:0.0`/`lesson_count:0`/`rule_count:0`；
  `worker.rs:33` 硬编码 evaluate 门槛 0.75,true,true,true,0.5…；`worker.rs:36` 硬编码 statement
  「Consolidated memory from reflection」。事件 payload 从不解码。
- 下游也不接：`context-engine/retriever.rs:41-48` `retrieve_reflections` 恒 `Ok(Vec::new())`（MVP 注释）；
  agent-engine 只读空 `reflections.len()`（agent-engine/src/runtime.rs:54）。反向：写入 `memory_index` 的唯一
  调用是 memory-engine 自己 promotion（promotion.rs:14-15）—— 反射**从不**直写 memory 表。
- memory cron 与 reflection cron 双双 feature-flag 关（wrangler.toml:23）。

### ⑤ 分类表

| # | 发现 | 类别 | 证据 | P |
|---|---|---|---|---|
| R-1 | **失败一次即永久死锁**：`UNIQUE(decision_id)`（0024:24）× 重试恒重新 `INSERT`（jobs/reflection.rs:100-107 → service.rs:98-102 → crud.rs:15）× `mark_failed` 拿 reflection_id 当 decision_id 查（service.rs:231-250，注释自认 pre-decoupling 遗留）→ 重试行 retry_count 恒 0 → `retry_count<3` cap 永不触发（crud.rs:135）→ 生成失败过的 decision 永久无法再反射，且 cron 每轮可从 eligible+failed 双路径重复撞约束（crud.rs:106-125 + 128-145 交集，jobs/reflection.rs 无去重）；API 路径同理：成功后再 POST 也 502 | **真实 lifecycle 死锁 + 重试 bookkeeping 错** | 代理 C 确认机制 | **P1** |
| R-2 | **引擎产物无任何读路径**：lessons[]/rules[] 只在 R2 artifact；`ArtifactRegistry::read`/`find_by_owner` 零生产调用且源码注释自认 KNOWN DEFECT（artifact_registry.rs:189-218，read 按 entity_id 查但 store 写 entity_id=0；find_by_owner 查错表）；HTTP 无 GET 反射路由（代理 A）；唯一读端 decision timeline 只取 `result` 摘要（application/services/decision_read.rs:172-184）；GraphProjectionService Reflection/Memory 节点声明但从不 emit（graph.rs:66-71 vs :119-212）；`retrieve_reflections` 恒空（retriever.rs:41-48） | **生产 gap：引擎产出不可观测、不可被下游消费**（learning loop 不闭合） | 代理 A | **P1** |
| R-3 | step6 artifact 持久化 best-effort：`artifact_result.ok()` 吞错（service.rs:165）+ 失败伪造 `memory/reflections/REF-*.json` 键**无人写**（全仓仅 service.rs:170 一个字面量）→ 完整内容静默丢失，只余 result+计数；R2 成功后 D1 行失败留孤儿 R2 对象（artifact_registry.rs:61-105 R2-first→D1，均 `?` 无重试）；无 reconciliation（D2-④ 族，本域无 ADR） | **事实丢失** | 代理 C | **P2** |
| R-4 | Reflection→Memory **查询 bug + 字段桩**：`find_event_keys("reflection","",…)` 空 aggregate_id 恒零行（memory_repository.rs:57）；candidate/worker 全硬编码 | 真实 lifecycle gap（④ 答案的具体行） | 代理 B | **P1** |
| R-5 | step8 legacy outbox 双写 = 不可归档 junk（payload 非 `EventEnvelope`，排程器 from_str 失败 archive.rs:75-81）；回归测试**钉死**双写（service.rs:430-471）；共享 drain `mark_outbox_failed` guard 反写 `AND retry_count>=3`（outbox.rs:69-81）+ 无 mark_outbox_retry → 毒行永 pending、每 cron 重排（波及所有失败 outbox 行，非反射专属） | 冗余 sink + 毒行 | 代理 C | **P2** |
| R-6 | completeness gate 死代码：`evidence_score=0.2` 硬编码 → min 0.5 > 0.4 阈值永不触发（context.rs:96-110 vs service.rs:130-134）；`mark_failed_with_retry`（写 retry=3）不可达；**后果**：API reflect 无资格校验 → 可对无 outcome 决策照常生成 | 防护失效（架构债务） | 一手 | **P2** |
| R-7 | 无 aggregate / 无状态机；lease/update/step8/step9 错误全 `let _` 吞（service.rs:105,120,200,210）；无 FK(decision_id)（0024 无 REFERENCES）；`outcome_id` 列恒 NULL（create 恒 None，infra reflection_repository.rs:36）——schema 声称关联实未写 | 架构债务（② 答案） | 一手 | **P2** |
| R-8 | 反射**重试轮 event_id 不撞**（不同 REF id → 不同 seq hash）；事件域内无 D2-③ 面 | 核过干净 | 代理 C | — |

**P3 / 暂不处理**：reflections.outcome_id 死列清理、typed `IntelligenceEvent::ReflectionGenerated` 变体从不
构造（shared-kernel/events.rs:224，事件全走 string DomainEvent）、reflection 行 FK 加固 —— 均待有对应重构/
迁移排期。

## 3. 与决策垂直（D2）的对照

| D2 类 | Reflection 现状 |
|---|---|
| hydrate 空 / snapshot-DB 错位 | N/A（行是真源，事件不可回放）—— 但镜像缺口：完整产物只在 R2 不可读（R-2/R-3） |
| 事件/事实丢失 | R-3（artifact 吞错 + 伪造键）、R-5（junk sink） |
| ID / 唯一性 | create 撞 `UNIQUE(decision_id)` → 重试死锁（R-1）；事件侧干净（R-8） |
| outbox best-effort | decision 是 ADR-006 收口；reflection 无 ADR，且 step8 sink 结构上不可归档（R-5） |
| 测试替身掩盖 | 两个实例：FakeRepo 掩盖 mark_failed 错 id（R-1）；双写回归测试钉死 junk sink（R-5） |

## 4. 附：诚实边界

- 「零行」「毒行」「不可归档」为静态推理（未连生产 D1 / 未跑真实 archive drain）。
- 未逐文件通读 model-runtime 各 provider 的生成行为；「AI_API_KEY 缺失 → Noop → 校验/解析失败」为
  Noop 返回固定文本 + parse_reflection_response 的推断，非实测。
- Signal 上游 A/B（上一份报告 §4）未在本轮触及；Reflection 与 Signal 共享的排程/事件/outbox 基础设施缺陷
  （如 mark_outbox_failed guard）标了波及面但按用户约束不动。

## 5. 修复记录（2026-09-06 — R-1 失败重试死锁）

用户从 §⑤ 分类选修 **R-1**（选项描述：「只修失败重试死锁：重试改为更新既有 failed 行 + 修正
mark_failed 的错 id 查找（严格沿现有代码，非重设计）。绿门禁后提交。」）。Signal / R-2..R-8 未动。

### 根因链（对照 §⑤ R-1 行）

- `reflections` 表 `UNIQUE(decision_id)`（mig 0024:24）→ 一个 decision 至多一行；
- 对同一 decision 重试 = 再次 `engine.execute` → 引擎 step1 恒 `repository.create`
  （service.rs:97-102）→ infra `create` 恒 `INSERT` 新行 → 撞唯一约束直接失败；
- `mark_failed`（旧 service.rs:231-250）拿 reflection 行 id 当 decision_id 去
  `find_latest_for_decision(id)` → D1 上 reflection id ≠ decision id → `None` → `unwrap_or(0)`
  → 重试行 retry_count 恒 0 → `retry_count < 3` cap（crud.rs:135）永不触发 → 决策失败过一次即
  永久无法再反射；
- scheduling 的 eligible（无行）与 failed（<3）两路径可对同一 decision 重复开跑，无去重。

### 改动（3 处行为 + 错误变体 + 5 个回归测试）

1. **`infrastructure/src/reflection_repository.rs` `create`**：先 `get_reflection_by_decision` ——
   若既有行 `status='failed' && retry_count < MAX_REFLECTION_RETRIES(=3)` → 用既有
   `update_reflection` 重置为 `generating` 并 **返回原 id**（重试 = 更新既有行，不再二次 INSERT）；
   其余状态（generated / generating / 超 cap）→ `Err(ReflectionError::AlreadyTracked(decision_id))`。
   cap 常量注释与 crud.rs 的 `<3` 同步。
2. **`reflection-engine/src/service.rs` `mark_failed(id, decision_id, error)`**：retry bookkeeping 改按
   **decision_id** 读 `find_latest_for_decision` → `retry_count+1` 写入；三处调用点（context_error /
   llm_error / validation_failed）补传 decision_id。
3. **`store/src/d1/reflection/crud.rs` `decisions_eligible_for_reflection`**：NOT EXISTS 去掉
   `AND r.status != 'failed'` → 任何已有 reflection 行的 decision 不再当 fresh 候选 → eligible 与
   failed 两表不相交，超 cap 行不再每轮被 eligible 反复重挑（cap 只在 `failed_reflections_for_retry`
   的 `<3` 一侧表达，与 infra 重开同界）。
4. `reflection-engine/src/error.rs`：+ `ReflectionError::AlreadyTracked(i64)` 变体。
5. 回归测试 5 个（workspace 404 → **409**）：
   - infra 4（`D1ReflectionRepository<MemoryStore>`，沿用 D2 harness）：
     `failed_row_under_cap_is_reopened_on_the_same_row` /
     `generated_or_generating_row_is_refused_not_overwritten` /
     `failed_row_at_or_over_cap_is_given_up_not_reopened`（cap 边界：2 重开、3 拒）/
     `fail_reopen_then_generate_keeps_one_row_with_original_id`；
   - engine 1：`failed_attempts_advance_retry_count_to_the_cap` —— 新 `RetryRepo` 忠实建模 D1
     UNIQUE+reopen 契约（替换原 FakeRepo「对任意入参返 Some(retry 0)」的替身掩盖，D2-① 同款），真引擎
     连跑 4 次：retry 1→2→3、每次 lookup 均按 **decision_id(42)** 而非行 id(0)、第 4 次 create 拒
     （cap 生效，不再无限重跑）。

门禁全绿：`cargo test --workspace` **409 passed / 0 failed**、`cargo clippy --workspace -- -D warnings`、
`cargo fmt --check`、`cargo check --workspace --all-features --target wasm32-unknown-unknown`、
`bash scripts/check-layered-deps.sh`（0 grandfathered）均过。

### 行为影响

- 重试从「一次失败即死锁」变为「同 id 重开、最多 3 次」：失败 → 下次 cron failed 表挑到 → create 重开
  同一行 → 成功写回同一行（无孤儿行 / 无重复行）；3 次全败 → 该 decision 放弃，cron 不再空转撞约束。
- API `POST /decisions/:id/reflect` 对已有 generated 行仍报错（语义保留：一个 decision 一份反射，
  撞 id 从「D1 unique 报错」变为「AlreadyTracked 明确报错」）。
- **残余（R-1 之外，按用户约束未动）**：context/llm 两条 `map_err` 里的 `mark_failed` future 仍被
  `let _` 丢弃而不执行（service.rs:125/138 —— 行留 `generating`，靠 stale recovery 翻转后进入 failed
  重试路径，与 cap 语义一致，但不落 last_error）；validation 失败路径（service.rs:146）是真 await、
  本次修复对其完全生效。
