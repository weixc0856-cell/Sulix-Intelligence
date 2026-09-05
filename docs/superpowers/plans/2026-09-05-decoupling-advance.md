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
| P4 remove `StoreBackend` | ❌ 未动（supertrait + d1_delegate 转发 + memory/backend 仍在） |
| P5 application 唯一用例入口 | ❌ 未动（application 仍薄，api 直接 `use store::`） |
| P6 清理旧 engine 壳 / 伪迁移层 | 🔒 GATED —— intelligence-domain 裁决 |
| P7 cargo-metadata 架构护栏 | 🟡 仅 heuristic 脚本，未入 CI |

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
