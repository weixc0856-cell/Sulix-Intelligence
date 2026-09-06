# Signal Engine / Intelligence-Loop 现状核验（2026-09-06）

> **触发**：Decision Vertical 封板后（P0 correctness CLOSED），用户拍板新主线按业务闭环上游优先：
> **Signal Engine 核验 → Reflection → Agent → 跨 vertical 集成**。本报告只核验现状、不写码
> （同 decision audit 约定），重点回答三个问题：
> ① Signal 有没有真正的 aggregate / lifecycle？
> ② Signal 是否真的能从 Observation/输入产生，还是只有 crate/interface？
> ③ Signal → Thesis/Claim 的连接是否真实，还是概念层？
>
> 核验范围：`signal-engine` / `intelligence-domain` / `claim-engine` / observation·claim·signal 相关
> store 读写 / worker-entry jobs·routes·cron / api 路由挂载。方法：一手读码 + 三路子代理
> 交叉取证（A: observation 生产真相；B: signal 输入源与 lifecycle 语义；C: claim/thesis 死活），
> 每结论挂 file:line。**未跑生产 D1 行数**（CF 操作受 allowlist 门禁）——"空表"均由"零生产者"
> 推理，非 `SELECT COUNT(*)` 实测；如需实测另行授权。

## TL;DR

Sulix 的"业务闭环"当前是一个**两套系统的拼接**，中间是断的：

- **真实在跑的**：RSS ingestion → `articles`/实体 →（cron 30min）旧 `signal-engine` `run()` →
  `signal_threads`/`intelligence_signals` + EventStore 事件；decision/outcome 经 API 手动；reflection/memory
  cron feature-flag。
- **只有 schema + 只读 API 的（无任何生产者）**：`observations`、`claims`、`claim_evidence`、
  `confidence_events`。观察 → 主张 → 信号的意图链路在生产端**零写入**。
- **意图 vs 现状的分层错位**：`intelligence-domain`（ADR-004 定案为 signal/claim 领域类型永居所）的
  `IntelligenceEngine` 是骨架（`observe` 真实但无 infra 端口实现；`analyze` 占位空；`detect_signals`
  恒 `NotFound`），且**全仓零生产消费者**；三个 repository 端口（Observation/Claim/Signal）在
  infrastructure **无 D1 实现**。

一句话：**Signal 本身是真的（每 30 分钟跑、落库、产生事件），但它由「文章 + 实体」SQL 聚合而来，
与 Observation/Claim 完全不相交；Observation → Claim → Signal 这段用户所画闭环的前半段在生产端不存在。**

## 1. 意图架构（docs 里的闭环） vs 生产现状（代码实际跑的）

### 1.1 用户/文档设想的闭环

```text
Observation → Signal → Thesis/Claim → Decision → Outcome → Reflection → Memory
```

意图证据：
- `intelligence-domain/src/observation.rs:1-4`：「Every piece of content entering Sulix passes through
  Observation. This is the root of the provenance chain.」
- `intelligence-domain/src/engine.rs:1-6`：IntelligenceEngine 是 deep module，内部管线
  extract → validate → confidence → signal。
- `final-architecture-v2.md` §4 + decoupling plan DoD #4：`intelligence_domain::` 是 signal/claim 领域类型
  **唯一来源**（ADR-004，2026-09-06 裁决保留为永居所）。
- `api/src/routes/observation.rs:51-66` lineage docstring 声称
  「Source → Observation → Signals → Claims → Decisions」provenance。

### 1.2 生产实际跑的（逐段状态）

| 闭环段 | 状态 | 关键证据 |
|---|---|---|
| **RSS ingestion → articles/实体/embedding** | ✅ **真实** | `runtime/queue.rs:58` / `runtime/cron.rs:33` → `jobs/ingestion.rs:117` → `d1/article.rs:62` `INSERT OR IGNORE INTO articles`；ingestion.rs:153 写 artifact_registry、entity 三表 |
| **Observation** | ⛔ **无生产者（空壳）** | `save_observation` 定义于 `d1/observation/crud.rs:12`，全仓**无生产调用**；`ObservationService`（`application/services/observations.rs:11`）只有 list/get/lineage 三个只读方法；api 只注册 GET（`api/lib.rs:88-90`）；`observations` 表迟至 migration 0034 才补建、无触发器；ingestion.rs:290 留 TODO「excerpt should go into an Observation」——作者自认未实现 |
| **Observation → Claim（analyze）** | ⛔ **占位桩** | `intelligence-domain/engine.rs:53-57` `analyze()` TODO 返回 `Ok(Vec::new())`；无任何调用方 |
| **Claim** | ⛔ **无生产者 + 三套模型不相通** | `INSERT INTO claims` 唯一在 `d1/claim/crud.rs:13`，其非 store 调用点 `application/services/claims.rs:46` 在 `#[cfg(test)]` 内；`attach_evidence`（claim_evidence）全仓零调用；`domain/models/claim.rs`（无 confidence 字段 DTO）vs `intelligence-domain/claim.rs`（有 confidence 域类型，仅 engine.rs 引用）vs `claim-engine/domain.rs`（仅 ClaimCandidate）三套并存、**无任何转换代码** |
| **Claim → Signal（detect_signals）** | ⛔ **桩** | `intelligence-domain/engine.rs:62-65` `detect_signals()` 恒 `Err(NotFound)`；`intelligence-domain/repositories.rs` 三个端口（Obs/Claim/Signal）声明实现在 `infrastructure/d1/`，但**实际无任何 infra 实现**，唯一是 engine.rs `cfg(test)` 假件 |
| **Signal（生产）** | ✅ **真实但输入 ≠ Observation/Claim** | `wrangler.toml:22,26` `CRON_SIGNAL_ENABLED="true"` + `*/30 * * * *` → `runtime/cron.rs:49-51` → `jobs/signal.rs:90 SignalEngine::run()`。候选源 = `EntitySignalSource` + `SemanticDiscoverySource`（`jobs/signal.rs:79`）＝ `article_entities JOIN entities JOIN articles` GROUP BY entity 5 因子评分（`d1/signal/candidate.rs:236-249`）+ 文章向量 ANN（`d1/article.rs:296-302`）；**store/d1/signal 与 signal-engine 内 grep claim/observation 零命中** |
| **Thesis** | ⛔ **不存在于代码/表** | 全仓无 thesis 表/迁移/实体；唯一近义物 `reflection-engine/context.rs:22-26` `ThesisSnapshot` 只是把 decision 的 `hypothesis` 改名装载 |
| **Decision** | ✅ **真实 aggregate**（D2 已封板） | decision-engine aggregate → `DecisionRepository`；D1 落 `decisions` 行 |
| **Signal → Decision 边** | 🟡 **真实但仅 API/人工，非引擎衍生、无 DB FK** | `decisions.signal_thread_id` 只是 nullable INT + 索引（`migrations/0014_decision_loop.sql:13-31` 无 FOREIGN KEY）；值来自 `POST /api/intelligence/signals/:id/decisions` URL 参数（`worker-entry/routes/decision_write.rs:199`）；反向查询 `d1/decision/query.rs:44-49` |
| **Outcome** | ✅ **真实**（decision vertical） | outcome_events / verdict（见 D2 review，P0-1/3 已修） |
| **Reflection** | 🟡 **真实但 signal-blind** | cron flag `reflection_enabled` → `jobs/reflection.rs`；决策下游（读 decision+outcome）；**不读 signal thread**（`reflection-engine/context.rs:100` 占位） |
| **Memory** | 🟡 **cron flag** | `runtime/cron.rs:65-67` `memory_enabled` → `jobs/memory.rs`（本核验未深入） |

### 1.3 已确认的死代码 / 过渡壳（本核验附带定位）

- **`claim-engine` = 死 crate**：唯一命名它的 Cargo.toml 是它自身；`LlmClaimExtractor`（llm.rs:11）/
  `ClaimExtractor`（extractor.rs:9）在 crate 外零调用（`FULL_REVIEW_REPORT.md:150` 已记 MEDIUM）。双标
  DEPRECATED（lib.rs banner Sprint 6.2D），依赖仅用于 `intelligence_domain::confidence`。
- **`intelligence-domain/lib.rs:14`「claim-engine re-export from here」是过时谎言**：claim-engine 用自己的
  domain.rs，没有 re-export。
- **`application/services/claims.rs:1-3` 声称「Claims are written by the Pipeline Agent」——该 Agent 在仓内不存在**。
- **`intelligence-domain/engine.rs` 注释「Phase 6 removes intelligence-domain」过期**：ADR-004 已裁决保留为永居所。
- `events` crate 的 `ObservationCreated` 事件类型只在测试构造（events/lib.rs:89/119）。

## 2. 三问答案

### ① Signal 有没有真正的 aggregate / lifecycle？

**没有 aggregate。Signal 是行模型 + DB 维护任务。**

- 写端是批处理编排：`SignalEngine::run`（signal-engine/lib.rs:59-165）＝ for-each-source 收候选 →
  upsert thread → append instance → dual-write 事件。无 `SignalAggregate`、无状态机、无命令方法。
- lifecycle 是 cron 周期末尾**4 条全表 bulk UPDATE**（`d1/signal/lifecycle.rs:8-22`）：
  `active→decaying` 当 `last_seen_at < now-7d`；`decaying→resolved` 当 `< now-14d`；
  `decaying→active`（复活）当 `>= now-3d`；`resolved→archived` 当 `< now-30d`。
- 事件（`SignalScoreChanged`/`SignalCreated`）是 persist **之后** dual-write 的旁观日志（lib.rs:82-157），
  **不驱动状态**；状态全在 `signal_threads.status` + `intelligence_signals` 行。
- `report.lifecycle_transitions` 恒 = 1（lib.rs:162），名不副实。

**后果**：Signal 的"状态"是批量扫描刷新，不是事件溯源/守护转移；与 decision-engine 的 aggregate 语义
（D2）是两个量级。对用户闭环主张（Signal 须先形成稳定 domain lifecycle，否则下游 Reflection/Agent
只是局部工程）——现状尚未达到。

### ② Signal 是否真的能从 Observation/输入产生，还是只有 crate/interface？

**分两层：**

- **Signal 本身真实产生**：cron 每 30 分钟真跑（CRON_SIGNAL_ENABLED=true），真落库、真写 EventStore
  （EventStoreSignalLog → outbox + event_archive_index + R2，drain worker 异步归档）。
- **但从「Observation/输入」产生——不是**。真实输入 = 文章+实体 SQL 聚合 + 文章向量 ANN，**与
  observations/claims 两套表完全不相交**。而 Observation 自己生产端零写入（见 §1.2），意图的
  `observe → analyze → detect_signals` 链中 detect_signals 是恒错桩、analyze 是空占位、IntelligenceEngine
  无生产消费者。**所以"Observation 是 provenance chain 的 root"这句 doc 级宣称，在生产端是无源之水。**

### ③ Signal → Thesis/Claim 的连接是否真实，还是概念层？

**概念层，生产端断裂：**

- **Thesis 不存在**（代码/表/迁移）；只有 reflection 把 decision.hypothesis 改名为 ThesisSnapshot。
- **Claim 无生产者**（§1.2），claim→signal 的 detect_signals 是桩；signal-engine 对 claim 零引用。
- **反向 decision→claim 预留也未接线**：`link_claim_to_decision`（写 decision_record_claims）无调用者；
  即便接了，claims 空表 LEFT JOIN 也只会产出 null。
- **真实存在的唯一边是 signal→decision（人工 API）与 decision→outcome→reflection（真实）**；二者之间
  claim/thesis 全缺。

## 3. Signal 读侧分叉（已修 2026-09-06）—— 附原始发现与严重度修正

**初始判断（第一版）说「timeline 预计恒空」—— 过重，予以修正**：`store.load_signal_detail` 的 timeline
由 **instances + created_at** 构建（`store/d1/signal/detail.rs:272-308`），**并非空**。真实缺口是：

- **EventStore 里的事件（`SignalScoreChanged`/`SignalCreated`）在任何一条 detail 路由上都未浮现** ——
  富化缺口，非空 timeline、非数据丢失。写端（`EventStoreSignalLog` → outbox + index + R2）与读端
  （`thread_detail` 走 SignalQueryService 但 event_log 恒 None → 静默落 D1 legacy 空兜底；`signal_detail`
  走 raw `store.load_signal_detail`，**根本无事件合并逻辑**）脱节。前端实际打的是 `signal_detail`
  （`intel-web/.../intelligence-api.ts:35` `/signals/:id`）。
- **`signal_evidence` 全仓无任何 `INSERT`**（只被读，thread.rs:130）→ 证据回读恒空（此项未改，见 §4）。

### 修复（2026-09-06，只此一项改码；Signal 上游 A/B 未动）

两条 detail 路由统一走 SignalQueryService + 接入 R2 event log：
- `signal-engine/query/mod.rs`：新增 `SignalQueryService::with_event_log`（此前构造恒不设 event_log）。
- `worker-entry/routes/signal.rs`：新增 `signal_event_log()`（写端同一 adapter）与 `load_signal_detail()`
  助手；`signal_detail`（`/signals/:id`）与 `thread_detail`（`/threads/:id`）都改经它 —— merge_signal_events
  的 R2 分支（detail.rs:53-73）首次真正可达。
- 回归测试 2 个：`thread_detail_merges_r2_events_when_event_log_attached` /
  `thread_detail_without_event_log_keeps_stored_timeline_empty`。全 workspace **404 passed**（402+2），
  clippy/fmt/wasm gate 全绿。

行为影响：`/signals/:id` 响应从 raw `SignalDetail`（analysis=null、无存储事件）变为统一读模型的
`SignalDetail`（补 `analysis` 规则摘要 + 合并 R2 事件）—— 与 `/threads/:id` 一致，消除读模型漂移；
JSON 字段是超集，前端 `SignalDetailDTO` 兼容。
- **三张"被读 API 撑着的空表"**：claims / confidence_events / observations 的 GET 路由均已挂载
  （api/lib.rs:73/75/88-90），背后表无生产者 → 生产返回空/404。

> ⚠️ 上述"恒空"是基于零生产者的静态推理，未实测 D1；上线前建议对
> `signal_events` / `signal_evidence` / `observations` / `claims` / `confidence_events` 跑一次 COUNT。

## 4. 开放分叉（供用户决策，本报告不改码）

核验的产出不是"还有多少代码可拆"，而是一个产品方向的岔路 —— **Signal 到底要以谁为上游**：

- **选项 A —— 文章/实体驱动的 Signal 即产品真相**：承认当前生产链路（ingestion → articles → entity
  signal）就是 Sulix 的"信号"，把 Observation/Claim 的意图层降级或拆除（删/闲置空表 + 只读 API + 死
  claim-engine + intelligence-domain 骨架），避免"root of provenance"的虚假宣称误导后续。
- **选项 B —— 把 Observation 补成真实上游**：在 ingestion 里真正落 observation 行（ingestion.rs:290
  的 TODO 正是这个），再决定 claim 提取是否/何时接线 —— 才谈得上用户所画闭环前半段。
- **无论 A/B**：① Signal 读侧时间线分叉（写 EventStore / 读旧 signal_events）值得先修或先明确；②
  intelligence-domain 三端口要么给 D1 实现、要么明确暂不实现（当前"注释说实现在 infra、实际没有"是
  契约假象）；③ 旧壳（claim-engine / signal-engine `run()` / 双 DEPRECATED banner）的 P6 收口方向
  （ADR-004 已定 intelligence-domain 为永居所，删除/迁移尚待排期）。

## 附：交叉验证边界（诚实清单）

- 空表结论 = 全仓符号 + 字面 SQL grep（无生产调用）推理，非 D1 实测。
- 未逐文件通读 briefing/memory/backfill job 的间接别名写（worker-entry 全仓对 claim/observation 零命中
  已很硬；decision outcome 写的是 outcome_events 非本表）。
- Reflection/Memory/Agent 只做了边缘定位，非本轮核验主体（按用户排序 Reflection 为第二优先、Agent 最后）。
