# Sulix Intelligence — 后端状态 + 路线图（2026-09-06）

> **范围**：仅后端本仓库 `D:\Project\Sulix Intelligence`（Rust workspace，29 crates）。
> 前端 `intel-web`（独立仓库）不在本文档范围。
> 本文档是 decoupling 主线收口后的**状态快照 + 后续计划**；权威历史见下节关联文档。

---

## 1. TL;DR 现状

- **decoupling 主线已收口**（2026-09-06，C1–C7，已 push `2b1ff69..da2c29a`）：
  `application → store` 正常依赖边 = 0，P7 guard `GRANDFATHERED = &[]`（空表，全硬禁）。
- **架构终局**：新 crate `domain`（infra-free 端口 + DTO + StoreError）与 `composition`（wiring-only，
  仅 `ProductionAppServices = AppServices<D1Store>` 一行）；`api → concrete-infra = 0`；
  application 只依赖 infra-free `domain`（store 降为 dev-dep，仅 MemoryStore 单测用）。
- **守卫矩阵全绿**：fmt / test（0 failed）/ clippy -D warnings / wasm32 --all-features /
  `check-layered-deps.sh`（0 grandfathered / 0 removable）/ architecture guard。
- **生产数据为空**：CF 五资源已重置（2026-09-05），D1/KV/Vectorize 空库空索引；
  **DeepSeek chat API key 未填** → summarizer 自停、`ai_summary` 空、无 embedding → 播种 + backfill 全被锁。
- **阻碍架构进一步收口的只剩两个 GATED 决议**：`intelligence-domain` 存废（D1）与 Decision vertical（D2）——
  都是领域语义重设计，不是机械搬迁，需用户拍板。

---

## 2. 已完成主线时间线（2026-08-21 → 2026-09-06）

| 阶段 | 内容 | 范围 / commit | 状态 |
|---|---|---|---|
| P0 | 基线恢复（fmt/clippy/test）+ T1 | 08-21 testing-plan | ✅ |
| P1 | 依赖栅栏 `check-layered-deps.sh`；GRANDFATHERED 13→10→8→7→**0** | 09-05 | ✅ |
| P2 | domain-owned ports 收口（Reflection/Memory/Signal/Context）；SignalEvidence 刻意无 port | `docs/architecture/p2-port-closure-2026-09-03.md` | ✅ |
| P3 | adapter 迁移：signal/context/reflection/memory 走 infra adapter；**decision = GATED exception**；ai-pipeline 解耦 store | 09-05 | 🟡（decision 除外） |
| P4 | `StoreBackend` body 45→…→**0** → 空 composite + `DecisionWriteStore`（4 GATED write 方法） | `8dabecb` | ✅ |
| P5 | Phase 1：Source/Entity 编排上收 application | `37de1fc..e781672` | ✅ |
| P5b | composition-root 注入（`Router::with_data`） | `414c1b4`+`e50b712` | ✅ |
| Phase 2 | Domain Lift：api 六条 concrete-infra edge → 0（AppServices bundle） | `461c520..2b1ff69` | ✅ |
| P6 | confidence 纯逻辑迁入 intelligence-domain；**完整 P6 范围 = GATED** | `acfaff8` | 🟡 |
| P7 | 跨 crate 架构守卫（cargo metadata）+ lint.yml `architecture-guard` job | `fcb728b` | ✅ |
| C1–C7 | **application:store 归零**：domain/composition crate、StoreError 去 worker、DecisionWriteStore、GRANDFATHERED 空表 | `ee065aa..da2c29a` | ✅ push |

> 09-05 的 `2026-09-05-decoupling-advance.md` §3/§4 里写的"下一步"（删 `application:store`、api:* 上收等）
> 已被 09-06 C1–C7 **supersede**；当时无独立计划文档记录 C1–C7，仅 CLAUDE.md 与
> `final-architecture-v2.md` §4 现状补记在案 —— 本文档即补上这一快照位。

---

## 3. 当前架构终局

```
application ──> domain（23 trait bound + DTO + StoreError，正常依赖，infra-free）
store ───────> domain（实现 domain::* + 再导出 `pub use domain::*`）+ worker（D1Database）
composition ─> application + store（一行 alias，wiring only，禁承载业务）
api ─────────> composition（只 import ProductionAppServices）+ 纯逻辑 crate（search/rules/content-governance）
worker-entry ─> composition + store（构造 D1Store；保留 Gate-B 直连 `env.d1("DB")` 的 infra 路由）
domain/application ─✗→ composition（防反向成环）
```

**P7 guard 规则**（`crates/shared-kernel/tests/architecture.rs`，`GRANDFATHERED = &[]`）：

1. domain crates 永不依赖 `worker` 或任何 concrete-infra（store/vectorize/embedding/event-store/object-store/infrastructure）。
2. application 永不依赖 `worker` 或 concrete-infra（store 仅 dev-dep，guard 忽略 dev 边）。
3. api 永不依赖 concrete-infra（delivery 可留 worker）。
4. domain 与 application 永不依赖 `composition`。
5. 工作区 crate 图无环。

**补充：两个 "domain" 概念并存** —— `crates/domain`（09-06 新建，persistence ports + DTO，机械搬迁产物）
与 `crates/intelligence-domain`（既有，领域类型 + confidence 纯逻辑）。命名/归属须由决议 D1 澄清。

---

## 4. 生产 / CI 基线

- **D1 migrations**：47 个（0001–0049 缺 0029/0030/0039），已全量重放于重置后的空库。
  唯一真源 = 根 `migrations/`；应用用 `wrangler d1 migrations apply sulix-feed-db --remote`（≠ deploy）。
- **CF 五资源**（2026-09-05 重置，Post-Reset Addendum 在 `docs/audit/cf-resource-conformity.md`）：
  D1 `ee083fd3-…` / KV `1cdea52318b4401391145b3898f68345`（写入 `crates/worker-entry/wrangler.toml` 单点真源）；
  Vectorize 重建 1024 维 cosine。**D1 与 Vectorize 现为空**。
- **CI gates**（lint.yml）：fmt / clippy -D / test / wasm32 gate / `architecture-guard` job；
  `cargo-deny check bans licenses sources`（**advisories 未启用**，因 fxhash unmaintained，见决议 D4）。
- **测试基线**：全量 `cargo test --workspace` = **379 passed / 0 failed**（2026-09-06 实测）。
  文档口径漂移：CLAUDE.md 记 346（2026-09-05），README 记 "350+/351" —— 均过期，Wave A 统一为 379。
- **wrangler `[build]` = 单一 Worker 构建入口**（ADR-003，已接受）；toolchain 1.97.0 由 rust-toolchain.toml 钉死。

---

## 5. 开放决议（每个都挡一条后续架构主线，需拍板）

### D1 — `intelligence-domain` 存废（P6 scope）
- **冲突**：`final-architecture-v2.md` §4 P6 写"删除 intelligence-domain"；但 decoupling 计划目标态图 +
  DoD #4（"`intelligence_domain::` 为唯一来源"）+ README + `acfaff8`（confidence **迁入**它）都指向**保留**。
- 裁决暂缓于 2026-09-05。现在 **`domain` crate 与之并存**，两个 "domain" 命名需一并澄清。
- **选项**：① 保留 → P6 语义改为"删旧 engine 壳 + `store::domain/*` 伪迁移层"（不含 intelligence-domain）；
  ② 删除 → 把类型/纯逻辑并入新 `domain` crate 后删壳（work 更大）。
- 挡：P6 收口 + README/两份 plan/架构文档同步。

### D2 — Decision vertical（GATED，最大的架构收尾）
- decision-engine 是"半成品域"：aggregate 缺 Serialize、outcome 未持久化、status 词汇不齐（`active` vs `Proposed`）、
  `save` 只 INSERT 无 upsert、`CreateDecision` 无 expected_outcomes。
- 生产写路径走 worker-entry 的 `DecisionService`（`S: StoreBackend` 空 composite，4 GATED write + insert_outbox）；
  application 的 `services/decision.rs::DecisionService`（`S: DecisionWriteStore`）与之撞名。
- **挡**：`StoreBackend` 空 composite 的最终删除（4 条 GATED 写方法 + insert_outbox 消灭）。
- 规则：裁决前**不动 production 决策路径**。域补齐后先切读路由、后切写路由（`2026-09-05-decoupling-advance.md` §5）。

### D3 — Vectorize 访问目标态
- custom `#[wasm_bindgen]` shim vs 上游/惯用 binding 契约 —— 架构层定夺（`cf-resource-conformity.md` rec #3）。
- 当前不挡主线；仅影响 vectorize crate 的长期维护面。

### D4 — cargo-deny advisories 启用
- 被 fxhash unmaintained（经 `scraper` 引入）阻塞。启用前需升级或替换 scraper（`deny.toml` / FULL_REVIEW）。
- 当前 `bans licenses sources` 已跑；`advisories` 是**补强**，不挡功能。

---

## 6. 后续开发路线图（wave 化）

### Wave A — 卫生与同步（无决议依赖，~0 风险，可立即做）
- FULL_REVIEW_REPORT §5 过期：测试数停在 08-21 基线（255/289、2.5/10 健康分）→ **重评健康分**
  （decoupling 已收口，"defer re-scoring" 到期）。
- 测试数 canonical 化：统一 README(EN/中文) 与 CLAUDE.md 为同一次全量运行口径。
- 把 C1–C7 收口正式补进 decoupling 计划（或本快照即为准）——消除 09-05 计划"下一步"被 supersede 的阅读歧义。
- 死代码清理：`DecisionQueryService::get_latest_evaluation` / `EvaluationQueryService::get_latest_evaluation`
  （P4 读端解锁后遗留的独立 backlog）。
- **DoD**：健康分重评 + 单一测试数 + docs 无过期引用；`rg get_latest_evaluation` 可解释归零。

### Wave B — 硬化 backlog（独立于决议，按价值排序）
- `.ok()` 静默绑定失败硬化（R2 / Vectorize / KV）—— 最高价值非紧急项（FULL_REVIEW P2）。
- `CRON_REFLECTION_ENABLED` / `CRON_MEMORY_ENABLED` 声明或删除（wrangler.toml 读而未声明，现默认 disabled）。
- KV namespace `title` 自文档化。
- FTS5 查询长度上限（P3，query 无 cap）。
- **DoD**：每项有测试或明确行为断言；clippy/test 全绿。

### Wave C — 决议驱动的架构收口（依赖 D1 / D2 拍板）
- **D2 通过后**：decision-engine 域补齐（aggregate 序列化 / outcome 持久化 / status 对齐 / save upsert /
  事件经 aggregate 发射 + `CreateDecision.expected_outcomes`）→ 两个 DecisionService 消歧 → 读路由先切、
  写路由后切 → 删 4 GATED 写方法 + `StoreBackend` 空 composite（P4 终局）。
- **D1 通过后**：intelligence-domain 定归宿 → P6 收口 + 三份文档同步。
- **search_articles（软议题，非违规）**：现为 worker-entry delivery 层直连 `env.d1("DB")` + `search::D1FtsSearch`，
  属**合规** infra 访问、无 api 边。仅当想统一 service/port 边界时才考虑再上收 —— 低优先，可由 D2 顺带定。
- Outbox 定位：当前 Reflection/Memory 双 port 上的显式 seam → 是否集中到 `shared/events`（Wave C 或 backlog）。
- **DoD**：D2 = `StoreBackend` 删除 + 写路由走 domain；D1 = 唯一 domain 概念落定 + docs 同步。

### Wave D — 测试补齐（testing-plan T6–T9 + adapter mapping gap）
- T6 application use-case 测试（decision/signal services；先确认 `MemoryArtifactRegistry` 测试替身是否存在）。
- T7 per-decoupling-commit 测试守卫（grep 数 `#[test]` 对比）。
- T8 **cross-domain 集成**（observe→claim→signal→decision→reflection，现为 0）→ `crates/application/tests/end_to_end.rs`。
- T9 delivery 层测试（api handler 解析、worker-entry cron/queue 分发）。
- **无 mapping 测试的 infra adapter**（执行前逐个核实）：`reflection_repository`、`memory_repository`、
  `context_repository`、`article_persistence`、`semantic_query`、`event_log`、`signal_event_log`。
  （有测试的：decision_repository / artifact_registry / provenance / storage_policy / signal_repository。）
- **DoD**：硬约束 —— 测试数量不能因迁移下降；T8 落地 ≥1 条 cross-domain 绿。

### Wave E — 运维放行项（需 DeepSeek key + 用户授权，非代码工作）
- 填 DeepSeek chat API key（当前 401 `Authentication Fails (governor)`）→ summarizer 重新启用。
- 重新播种 feeds/articles → embedding backfill（触发 model 调用，须放行，遵循 `sulix-cf-resource-policy`）。
- 确认 remote D1 migration 状态（ADR-003：`wrangler d1 migrations list --remote`，现只验过 local）。
- **DoD**：生产 smoke 绿；`ai_summary` / vector 非空。

---

## 7. 建议执行顺序

1. **Wave A（现在可做）**：无决议依赖，顺手把健康分与测试数修到一致。
2. **D1 / D2 决议**：交用户拍板（是继续架构收口的唯二门闩）；决议前不动 production 决策路径、不动 intelligence-domain。
3. **Wave C（D2 通过后）**：Decision vertical —— 唯一剩的大型架构收尾，且解锁 `StoreBackend` 删除。
4. **Wave E**：等你填 key + 放行 —— 与代码 wave 正交，随时可插。
5. Wave B / D 可在决议等待期并行推进（皆不依赖 D1/D2）。

---

## 8. 关联文档

- decoupling：`docs/superpowers/plans/2026-08-21-architecture-decoupling-plan.md`、
  `docs/superpowers/plans/2026-09-05-decoupling-advance.md`、`docs/architecture/final-architecture-v2.md`（§4 现状补记）
- 测试：`docs/superpowers/plans/2026-08-21-testing-plan.md`、`docs/testing.md`
- 审计 / 运维：`docs/audit/cf-resource-conformity.md`、`docs/ops/cf-reset-2026-09-03/CHAT_API_KEY_PENDING.md`、
  `docs/architecture/p2-port-closure-2026-09-03.md`
- 治理：根 `CLAUDE.md`、`docs/FULL_REVIEW_REPORT.md`、`deny.toml`
- 本快照由探索 agent 扫描 docs + git 历史 + 记忆核对产出（2026-09-06）。
