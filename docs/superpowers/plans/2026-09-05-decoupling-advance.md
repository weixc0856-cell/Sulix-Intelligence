# Decoupling Advance Plan (2026-09-05 — 续 decoupling plan P1–P7)

续《[2026-08-21-architecture-decoupling-plan.md](./2026-08-21-architecture-decoupling-plan.md)》。
2026-09-05 状态遍历 + decision recon 后，用户裁决：**主线 = 继续 DDD 去耦，P4 先行**；
**decision 接线不构成干净一步 → 记为 gated vertical**；intelligence-domain 存续裁决仍暂缓
（矛盾已标注于 final-arch-v2 §4）。

---

> **2026-09-06 主线收口补录（C1–C7，已 push `2b1ff69..da2c29a`）**：本节 §0 的 P4/P5 行记录的是 09-05
> 中间态。09-06 已完成**主线最后一步**：`application → store` 正常依赖边 = 0 —— 23 trait bound + DTO +
> `StoreError` 迁入新 infra-free `domain` crate（store 实现并 `pub use domain::*` 再导出），store 降为
> application dev-dep（仅 MemoryStore 单测）；新增 wiring-only `composition` crate
> （`ProductionAppServices = AppServices<D1Store>`），api/worker-entry 改指它（`api → concrete-infra = 0`
> 保持）。application 死 `DecisionService` 收窄到 `DecisionWriteStore`（§5 GATED 4 write 方法承载，
> SQL/契约/outbox-first 零改动），`StoreBackend` 保留为空 composite（worker-entry 生产 DecisionService
> 仍用，body = 0）。P7 guard `GRANDFATHERED = &[]`，新增 domain/application → composition 禁边。
> StoreError 去 worker（3 行 `impl From<worker::Error>` 删除，store 本地 `.s_err()` 承接）。
> 状态与路线图见 `docs/status-roadmap-2026-09-06.md`；CLAUDE.md 已同步。本计划 §0 以下各节为 09-05
> 执行记录，阅读时以收口后状态为准。

---

## 0. 现状基线（2026-09-05，遍历 + recon 核对，非仅文档）

| 项 | 状态 |
|---|---|
| `GRANDFATHERED`（`scripts/check-layered-deps.sh`） | **= 0**（13→10→8→7→0） |
| P0/P1 baseline + dependency fence | ✅ done |
| P2 domain-owned ports | ✅ done（Reflection/Memory/Signal/Context；p2-port-closure 2026-09-03） |
| P3 adapter migration | 🟡 **signal/context/reflection/memory 已走 infra adapter 并接线**；decision 例外（见 §5） |
| P4 remove `StoreBackend` | 🟡 **Phase A（45→19）+ Phase B 收窄（19→8）+ 读端解锁收尾（8→4）已完成（2026-09-05）**：signal/context/reflection/memory/outbox/event/memory-articles/rule/artifact/article-analysis 全拆出 supertrait；3 个 generic `S: StoreBackend` 消费方已收窄；**决策读端 4 方法（get_decision/…outcomes/…evaluations/latest_evaluation）已删，读 surface 全走 subtrait（§3 记录）**；`StoreBackend` body 现**仅余 4 个 GATED decision 写方法（§5 GATED）**。终局删除 supertrait + P5 见 §3 |
| P5 application 唯一用例入口 | 🟡 **Phase 1（Source/Entity 上收 application）+ P5b（composition-root 注入）均已完成（2026-09-05，记录见下文）**：业务编排在 `services/{sources,entities}.rs`；HTTP `Store` 由 worker-entry `runtime/http.rs` composition root 构造、经 `Router::with_data` 注入（`RouteContext<Store>`，handler `ctx.data.clone()`）。`api → store` Cargo 边**仍存**（P5b 只改 Store 构造位置、不改依赖图，P7 `GRANDFATHERED` 未删）；`search_articles` direct-D1 = Phase 2 例外。`StoreBackend` supertrait 仍存（body=4 GATED decision 写方法） |
| P6 清理旧 engine 壳 / 伪迁移层 | 🔒 GATED —— intelligence-domain 裁决 |
| P7 cargo-metadata 架构护栏 | ✅ done（`fcb728b`）：`shared-kernel/tests/architecture.rs` + lint.yml 独立 `architecture-guard` job |

## 1. 目标与边界

DDD 单向依赖 `Delivery → Application → Domain ↑ Ports ↑ Infrastructure`。推进 P4 → P5 → P7；
P3-finish（decision）单独裁决。**不变量**：测试数不减；`GRANDFATHERED` 保持空；每迁移 before tests →
migrate → after tests；fmt/clippy/wasm 绿。

---

## 2. Phase A — P4：已 port 上下文先行拆 `StoreBackend`（当前主线）

**recon 结论（2026-09-05，decision 上下文）**：`D1DecisionRepository` 生产零接线；production 写路径走
`api::services::decision::DecisionService`（直连 store + `insert_outbox`）；decision-engine 为半成品域
（aggregate 无 Serialize / outcome 无持久化 / 状态词汇 `active` vs `Proposed` 不对齐 / `save` 仅 INSERT /
`CreateDecision` 无 expected_outcomes）。**强切写路径 = 丢 outbox 事件、丢返回行语义、丢字段 → 行为风险。**
故 P4 从**已完整 port 的上下文**（signal / context / reflection / memory，其 store 方法已被 infra adapter
接管）开始，逐域拆掉 supertrait 方法；decision 接线单列 §5。

**P4 Recon（动代码前必做，产出为准）**
1. `crates/store/src/backend.rs` supertrait 方法按 bounded context 分组（signal/context/reflection/
   memory/decision/outbox/…）。注意：`#[deprecated]`、"no new methods" 只约束新增，不约束存量。
2. **关键**：infra adapters（`D1SignalRepository` / `D1ContextRepository` / `D1ReflectionRepository` /
   `D1MemoryRepository` 及 `D1DecisionRepository`）当前 `impl<S: StoreBackend>` 是否经 supertrait 调 store
   方法？若是，拆除前须先把 adapter bound 从 `StoreBackend` 收窄到对应 small trait
   （`store::traits/repo/*`、`traits/query/*`）或 `D1Store` 具体类型——这是 P4 的真工作量。
3. 逐域找出 supertrait 方法的全部调用点（generic `S: StoreBackend` 消费方）：infra adapters、
   application services、api、worker-entry、tests。分三类：(a) 零生产调用 → 可直接删；(b) 仅 infra
   adapter 经 bound 调用 → 收窄 bound 后可删；(c) 仍被 api/application 直连 → 先迁移调用方。
4. MemoryStore（memory/backend.rs）是测试替身：确认其实现的 supertrait 方法删除后测试是否仍覆盖
   （对应域 infra adapter 的映射测试是否已就位，能接管）。

**Actions（分 commit，逐 bounded context；无生产消费方的 supertrait 方法直接删，不建 speculative port）**
1. 收窄 infra adapter bound（`StoreBackend` → 小 trait / 具体 store），消除其对 supertrait 的依赖。
2. 迁移残余 generic 消费方；删除对应 supertrait 方法 + `d1_delegate.rs` 的 supertrait impl 段 +
   `memory/backend.rs` 对应 impl（若 MemoryStore 测试已由 adapter 映射测试接管）。
3. 终态：`backend.rs` supertrait 方法数收敛到仍被 generic 消费方使用的集合，再整体拆 supertrait（见 §3）。

**Verify / Commit**
- 每步 `cargo test --workspace` 不降；`-D warnings`；`scripts/check-layered-deps.sh` 空表。
- Commit 粒度：每 bounded context `refactor(store): drop StoreBackend <domain> methods`。

### Phase A 完成记录（2026-09-05，已 push `b7a51f9..f6dcbda`）

用户裁决 Option A（`store::traits` 新建细粒度 trait）。逐 batch：新 trait → D1Store + MemoryStore 双 impl →
挂入 `StoreBackend` supertrait 列表 → 删 supertrait 方法声明/impl body → 收窄 infra adapter bound。

| Batch | Commit | 拆除（StoreBackend body） | 新增 fine-grained trait | 收窄 |
|---|---|---|---|---|
| 0 | `c3c4ea9` | 9 方法（4 死代码 + 5 inherent-only） | — | — |
| 1 | `5ab158f` | outbox 4 / event-index 2 / memory 2 | `OutboxStore` `EventIndexStore` `MemoryPersistence` | memory_repository + event-store r2 |
| 2 | `5a9ce4a` | context 1 | `ContextSnapshotStore` | context_repository（→ DecisionQueryService 等） |
| 3 | `6f0b572` | reflection 3 | `ReflectionPersistence` | reflection_repository（+ swap decision read slices） |
| 4 | `f6dcbda` | signal 8（B1–B3 后余量） | `SignalStore` | signal_repository（`SignalStore`/`+ArticleQueryService`/`+SignalQueryService`）+ event-store d1_backend |
| 5 | `c4682a5` | 17 死/仅具体消费方法（expire_old_articles、artifact put/get、claim/observation/confidence/source 全套） | — | — |

`StoreBackend` body 方法数 45→36→**19**（2026-09-05 Phase B batch 5，已 push）。Batch 0–5 后仅剩：
rules（active_rule_jsons）/ article 生命周期（insert_article, set_ai_summary, set_raw_content_r2_key）/
feed（record_fetch_result）/ entity 别名（upsert_entity, link_article_entity, link_entity_relation）/
**decision+outcome+evaluation 8 方法（§5 GATED）** / artifact registry 3 方法。11 个非 decision 方法仍由
3 个 generic `S: StoreBackend` 消费方持有：article_persistence、artifact_registry、worker-entry
`FeedContext<S: StoreBackend>`（仅具体 store 构造处使用，见 §3）。
测试 **333 passed / 0 failed**（不降）；guard 空表；fmt/clippy/wasm 全绿。

**下一步**：Phase B 续（§3）收窄上述 3 个 generic adapter → 目标 `StoreBackend` 只余 8 个 decision
方法（GATED 不动），随后 Phase C（P7 cargo-metadata 守卫）。

---

## 3. Phase B — P4 终局 + P5 application 唯一用例入口

StoreBackend supertrait 方法清到不再有 generic 消费方后，删除 supertrait 定义；随后 P5：
- `api` handler 业务编排上收 `crates/application/src/services/*`；`api`/`worker-entry` 改调 application。
- 收敛：`api → store = 0`；`grep -R "use store::" crates/api` = 0。
- 注意 api 与 application 各有一个 `DecisionService` 撞名 —— P5 收敛时需消歧（重命名/合并）。

**Commit**：`refactor(store): delete StoreBackend supertrait` / `refactor(api): route use-cases through
application services (P5)`。

---

## 4. Phase C — P7 cargo-metadata 架构护栏（可与 P4/P5 并行）

- 建 `crates/shared-kernel/tests/architecture.rs`（不引第三方依赖，用 `cargo metadata` JSON）。
- 断言 forbidden edges：domain→store/worker；application→worker；api→infrastructure/vectorize/
  embedding/event-store/object-store；无循环依赖。
- 接入 `.github/workflows/lint.yml` 独立 job。

**Commit**：`test(governance): architecture dependency guard (P7)`。

### Phase B（收窄）+ Phase C 完成记录（2026-09-05，已 push `3eccd42`）

| Batch | Commit | 内容 |
|---|---|---|
| 6 | `ca4d4ee` | 新 `ArticleAnalysisStore` → 收窄 `D1ArticlePersistence`（body 19→17） |
| 7 | `847fc29` | 新 `ArtifactStore` → 收窄 `D1ArtifactRegistry`（body 17→14） |
| 8+9 | `a406b82` | worker-entry `FeedContext<S: StoreBackend>` → 具体 `store::Store`（ingestion + queue 两处）；`record_fetch_result` 归入 `FeedRepository`；新 `RuleStore`；删 `insert_article`+3 entity 别名（body 14→**8**，仅余 GATED decision 方法） |
| C | `fcb728b` | P7 `shared-kernel/tests/architecture.rs` + lint.yml `architecture-guard` job（333→334 tests 全绿） |

收窄后 `StoreBackend` body = **8 个 decision 方法**（create_decision/get_decision/update_decision_status/
create_outcome/get_decision_outcomes/create_evaluation/get_decision_evaluations/get_latest_evaluation，§5 GATED）。
测试 334 passed / 0 failed；guard 空表（0 grandfathered / 0 removable）；fmt / clippy / wasm 全绿。

**下一步**：P5 Phase 1（Source/Entity 上收，见下）已完成；续 **P5b composition-root 注入**（worker-entry
建 store 经 `Router::with_data` 注入 → 真正 `api → store = 0`；届时删 P7 guard `GRANDFATHERED` 中
`api:*` 与 `application:store` 条目即自动收紧）。

### P5 Phase 1 完成记录（2026-09-05，已 push `37de1fc..e781672`）

api 编排上收 application、确立可复用模式。本轮取 Source + Entity 两个**已完整 port** 的域（subtrait
全覆盖，store 零改动）；Article 读非零迁移（search/R2/content-governance 未 port）推后到 Phase 2。

| Commit | 内容 |
|---|---|
| `37de1fc` | `refactor(application): extract Source use-cases from api handlers` —— `services/sources.rs` `SourceService<S>`（`S: SourceQueryService + SourceRepository`，list/get/create/update/delete）；`crates/api/src/routes/source.rs` 5 handler 委托。update 保留 `feed_id` 不变量移入 service（D1 `save_source` upsert 走 `ON CONFLICT(feed_id)`，丢 feed_id 即插 orphan 行） |
| `6ee3ad9` | `fix(store): MemoryStore::save_entity 换 host-safe SystemTime 时钟` —— 原 `js_sys::Date::now()` 在 native `cargo test` 会 panic；对齐 `create_artifact` 既有 host-safe idiom（store 面唯一 js_sys 残留） |
| `e781672` | `refactor(application): extract Entity use-cases from api handlers` —— `services/entities.rs` `EntityService<S>`（`S: EntityQueryService`，list/get/relations/articles/activity + `const ACTIVITY_WINDOW_DAYS: u32 = 7`）；`crates/api/src/entities.rs` 5 handler 委托。`now` 由 handler 用 `js_sys` 算好后**作参数传入**（application 零 runtime/HTTP/js_sys） |

测试 **346 passed / 0 failed**（334 + Source 6 + Entity 6，净新增覆盖 —— 此二域此前零单测）；
MemoryStore 测试模式：先 seed 再 `XxxService::new(store)` 包同一个 store（move 后不可再借 store 写）。
guard 空表（0/0）；fmt / clippy -D warnings / wasm / P7 architecture-guard 全绿。

**此轮边界**：api handler 仍 `Store::new(ctx.env.d1("DB")?)` 自建 store（transitional）——`api → store`
未归零，composition-root 注入 = 独立后续阶段 **P5b**；`StoreBackend` supertrait 仍存（8 个 GATED
decision 方法），终局删除留待 decision vertical（§5）。

**下一步（续主线）**：P5b composition-root；Phase 2 域（Article 读需先定 search/R2/content-governance 的
port 边界、Feed、Rules CRUD、briefing、compliance/takedown、trust-stats）；§3 终局删 `StoreBackend`
supertrait（GATED）。

### 决策读端解锁完成记录（P4 收尾，body 8→4；未 push，等用户确认）

读端（非 GATED）4 方法已有独立 subtrait surface，先迁移生产调用点、再删 body surface —— 本轮只做
**读解锁**，4 个 GATED 写方法与 decision vertical（§5）一律不动。

| Commit | 内容 |
|---|---|
| `272c5e2` | `refactor(api): read decisions via subtraits` —— 迁移全部读调用：`routes/decision.rs` `get_decision`→`DecisionRepository::find_decision`（detail/timeline/explanation）；outcomes/evaluations → `<Store as store::DecisionQueryService>::list_outcomes/list_evaluations`（UFCS，因两方法各在 2 个 subtrait 上；沿 `graph.rs` 既有风格）；`api services/decision.rs` write-path read-back 改 `find_decision`（`S: StoreBackend` bound 不变，`find_decision` 已在 supertrait 上）；`infrastructure D1DecisionRepository::find` 改 `find_decision`；**`application graph.rs` `GraphProjectionService::expand` bound 收紧** `DecisionQueryService+OutcomeQueryService+StoreBackend`→`DecisionRepository+OutcomeQueryService`（same-migration bound tightening：实际只用到这两者） |
| `1a77cfc` | `refactor(store): retire decision read methods from StoreBackend body (8→4)` —— 删 `backend.rs` 4 个 body 读方法（get_decision/get_decision_outcomes/get_decision_evaluations/get_latest_evaluation）+ `d1_delegate.rs` 4 个 `impl StoreBackend for D1Store` read delegates + `memory/backend.rs` 4 个 `impl StoreBackend for MemoryStore` body impl。**未删**：underlying SQL / 各 subtrait impl / MemoryStore state（`find_decision` 仍 delegate 到同一 D1 lookup） |

迁移后读 surface 全走：`DecisionRepository::find_decision` + `DecisionQueryService::list_outcomes/
list_evaluations` + `OutcomeQueryService::list_outcomes` + `EvaluationQueryService::list_evaluations`。

`StoreBackend` body = **4 个 GATED 写方法**：create_decision/update_decision_status/create_outcome/
create_evaluation。`get_latest_evaluation` 0 调用点，无迁移直接删。测试 **346 passed / 0 failed**（不增不减）；
guard 空表（0/0）；fmt / clippy -D warnings / wasm / P7 architecture-guard 全绿。structural acceptance：
body=4 确认；api/application/infrastructure 对 4 个旧读方法生产调用 = 0。

**方法解析要点（复用时注意）**：concrete 类型调 trait-only 方法需 trait in scope（`routes/decision.rs`
补 import `DecisionRepository`）；generic 只需 where bound；同名跨 subtrait 方法用 UFCS 消歧。

**边界**：`DecisionQueryService::get_latest_evaluation` / `EvaluationQueryService::get_latest_evaluation`
仍为 dead code，独立 cleanup backlog（§7），本轮未清；decision vertical（§5）仍整体 GATED。

**下一步**：P4 最后一个非-GATED shrink 收口。后续不再从 `StoreBackend` 搬方法，而是集中解决 decision
write vertical 的 domain/persistence contract（§5），再一次性消灭剩余 4 个 GATED body 方法 + 删
`StoreBackend` supertrait（§3 终局）。

### P5b composition-root 注入完成记录（2026-09-05，已 push `414c1b4` + `e50b712`）

api handler 自建 `Store::new(ctx.env.d1("DB")?)` 上收 worker-entry HTTP runtime（composition root），
经 `Router::with_data(store)` 注入 `RouteContext<Store>`。本轮**只改 Store 获取位置**，不改变 Cargo
依赖图：`api → store` 仍存（P7 `GRANDFATHERED` 不删，删边留待 Phase 2 上收）；cron/queue jobs 保持
自建 store。

| Commit | 内容 |
|---|---|
| `414c1b4` | `refactor(store): make D1Store cloneable for router injection` —— `D1Store.db: D1Database` → `Arc<D1Database>` + `#[derive(Clone)]`；domain SQL 走 Deref 零改动 |
| `e50b712` | `refactor(worker-entry): build Store at composition root and inject via Router::with_data` —— `api::router(store) -> Router<'static, Store>`（`Router::with_data` 起步，因 `Router::new()` 仅在 `Router<()>` 上）；api + worker-entry HTTP internal 全部 handler `RouteContext<()>`→`RouteContext<Store>`、`Store::new(ctx.env.d1(...))`→`ctx.data.clone()`（~90 处 / 24 文件）；`param_i64<D>` 泛型化；`decision build_decision_service` / `reflection build_engine` 改收 store 参数（reflection 3×D1Store 构造 → store.clone）；`today_briefing` 删 per-handler D1-binding 503 |

**§9 行为裁决（D1 = HTTP router 统一前置依赖）**：HTTP 全路由仅 `cors_preflight`/`ping`（system.rs）
不读 D1；二者留 store-injected router 上（签名改、body 忽略 `ctx.data`）。理由：`env.d1("DB")` 仅本地
binding 解析（D1 宕机表现为查询错误而非 binding 失败）；binding 未配置 = worker 整体不可用，此时 503
暴露配置错误优于 200 掩盖。two-router 拆分（无 D1 router 先跑、404 落回）记入 §7 backlog。
`today_briefing` KV/R2 cache-first 路径在 D1 binding 未配置时被前置 503 挡住 —— 同上可接受，已记录。

**Phase 2 exception**：`search_articles` 仍 `ctx.env.d1("DB")?` → `D1FtsSearch`（search-port 属 Phase 2
Article 读，不在 P5b 顺手做）—— Gate B 唯一授权例外。

structural acceptance（Gate A/B/C）：api 内 `Store::new`/`D1Store::new` = 0（仅 R2Store/EventStoreLog
等非 D1 构造）；api 内 `env.d1("DB")` = 仅 article.rs `search_articles`；worker-entry `env.d1` =
http.rs composition root + jobs/queue（cron 保持现状）。测试 **346 passed / 0 failed**；guard 空表
（0/0）；fmt / clippy -D warnings / wasm / P7 architecture-guard 全绿。

**下一步**：Phase 2 域上收（Article 读先定 search/R2/content-governance port 边界、Feed、Rules CRUD、
briefing、compliance、trust-stats）→ 届时删 P7 guard `GRANDFATHERED` 中 `api:*`/`application:store`
条目即自动收紧。

---

## 5. Phase D — Decision vertical（GATED，不并入 P4 主线）

recon（本文件 §2）证明 decision 接线是**半成品域迁移**而非机械去耦步。像 p2 的 SignalEvidence gate 一样
**记录为待决议 vertical**：当 decision-engine 域补齐（aggregate 序列化/outcome 持久化/status 词汇对齐/
`save` upsert/事件经 aggregate 发射 + 两个 `DecisionService` 消歧）后，另立 Phase 执行：
读路由（list/detail/by_signal）先切 → 写路由后切。裁决前不动 production 决策路径。

---

## 6. Phase E — P6（GATED）+ 收尾文档同步

P6 等 intelligence-domain 裁决。收尾（Task 7.2 基线）：更新 `CLAUDE.md` 架构/依赖节、
`FULL_REVIEW_REPORT.md` §5、归一 sprint 编号叙事（独立 docs commit）。

---

## 7. 相邻积压（tracked，不在本线内推进）

重灌 feed/文章 + embedding backfill（需 DeepSeek key，用户约束先不调）；`.ok()` 硬化；
`CRON_REFLECTION/MEMORY` vars；KV `title`；历史资源清理（独立授权）；intelligence-domain 裁决；
decision vertical（§5）；Vectorize 惯用法目标态；`mark_failed` 语义；outbox 迁移。

---

## 8. 完成定义（阶段性）

- P4：signal/context/reflection/memory 的 supertrait 方法移除；无 generic `StoreBackend` 消费方残留于
  已 port 域；`StoreBackend` 终局可删；测试不降、guard 空表、fmt/clippy/wasm 绿。
- P5：`api → store = 0`；application 覆盖所有用例；`DecisionService` 撞名消歧。
- P7：cargo-metadata 架构守卫入 CI 且绿。
- P6 / Decision vertical：仅按各自裁决后执行。

## 9. Gates

每 Phase = 独立 commit。**push = BLOCKED，须用户明确确认**（沿用记忆 `sulix-cf-resource-policy`）。
纯代码，不触碰 Cloudflare allowlist/denylist；`wrangler.toml` 不改。
