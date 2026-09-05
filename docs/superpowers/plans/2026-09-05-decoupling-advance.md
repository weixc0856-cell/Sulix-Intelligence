# Decoupling Advance Plan (2026-09-05 — 续 decoupling plan P1–P7)

续《[2026-08-21-architecture-decoupling-plan.md](./2026-08-21-architecture-decoupling-plan.md)》。
2026-09-05 状态遍历 + decision recon 后，用户裁决：**主线 = 继续 DDD 去耦，P4 先行**；
**decision 接线不构成干净一步 → 记为 gated vertical**；intelligence-domain 存续裁决仍暂缓
（矛盾已标注于 final-arch-v2 §4）。

---

## 0. 现状基线（2026-09-05，遍历 + recon 核对，非仅文档）

| 项 | 状态 |
|---|---|
| `GRANDFATHERED`（`scripts/check-layered-deps.sh`） | **= 0**（13→10→8→7→0） |
| P0/P1 baseline + dependency fence | ✅ done |
| P2 domain-owned ports | ✅ done（Reflection/Memory/Signal/Context；p2-port-closure 2026-09-03） |
| P3 adapter migration | 🟡 **signal/context/reflection/memory 已走 infra adapter 并接线**；decision 例外（见 §5） |
| P4 remove `StoreBackend` | 🟡 **Phase A（batch 0–5，45→19）+ Phase B 收窄（batch 6–9，19→8）已完成（2026-09-05）**：signal/context/reflection/memory/outbox/event/memory-articles/rule/artifact/article-analysis 全拆出 supertrait；3 个 generic `S: StoreBackend` 消费方（article_persistence/artifact_registry → 小 trait，worker-entry FeedContext → 具体 `store::Store`）已收窄；`StoreBackend` body 现**仅余 8 个 decision 方法（§5 GATED）**。终局删除 supertrait + P5 见 §3 |
| P5 application 唯一用例入口 | 🟡 **Phase 1（Source/Entity 上收 application）已完成（2026-09-05，记录见 §3）**：业务编排移入 `services/{sources,entities}.rs`，api 委托；`api → store` 减少未归零（handler 仍自建 `Store::new`，composition-root 注入 = P5b）。`StoreBackend` supertrait 仍存（body=8 GATED decision）。P7 guard `GRANDFATHERED` 已记 `application:store` + `api:*` 8 边（删边即收紧） |
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
