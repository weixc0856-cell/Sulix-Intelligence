# Decoupling Advance Plan (2026-09-05 — 续 decoupling plan P1–P7)

续《[2026-08-21-architecture-decoupling-plan.md](./2026-08-21-architecture-decoupling-plan.md)》。
2026-09-05 经项目状态遍历确认真实进度，用户裁决：**主线 = 继续 DDD 去耦**；
**intelligence-domain 存续裁决暂缓**（矛盾已标注于 final-arch-v2 §4 与本 plan 目标态图）。

---

## 0. 现状基线（2026-09-05，遍历核对，非仅文档）

| 项 | 状态 |
|---|---|
| `GRANDFATHERED`（`scripts/check-layered-deps.sh`） | **= 0**（13→10→8→7→0） |
| P0 baseline / P1 dependency fence | ✅ done |
| P2 domain-owned ports | ✅ done（Reflection/Memory/Signal/Context；p2-port-closure 2026-09-03） |
| P3 adapter migration | 🟡 部分（signal/context/ai-pipeline 已 port 到 infra；**`D1DecisionRepository` 未接线**） |
| P4 remove `StoreBackend` | ❌ 未动（`store/src/backend.rs` supertrait 仍存在，d1_delegate 转发） |
| P5 application 唯一用例入口 | ❌ 未动（application 仍薄，api 仍直接 `use store::`） |
| P6 清理旧 engine 壳 / 伪迁移层 | 🔒 **GATED** —— 依赖 intelligence-domain 裁决 |
| P7 cargo-metadata 架构护栏 | 🟡 仅 heuristic 脚本，未入 CI |
| Task B（P6 置信度归域 + CF 审计 + 5 资源重置） | ✅ closed（`acfaff8` / `62ffd8e`） |

**基础设施 infra 实况**（供 P3-finish 精确范围）：`crates/infrastructure/src/` 已含
`signal_repository.rs` / `context_repository.rs` / `memory_repository.rs` /
`reflection_repository.rs` / `decision_repository.rs` / `event_log.rs` /
`signal_event_log.rs` / `semantic_query.rs` / `article_persistence.rs` 等。
**不在本线建**（p2-port-closure 已记录、有意为之）：`SignalEvidenceRepository`（evidence 无独立生命周期）、
Claim/Observation/Framework adapters（无生产消费方 / decorative）。

---

## 1. 本线目标与边界

**目标**：把 frozen `final-architecture-v2.md` §4 的 DDD 单向依赖（
`Delivery → Application → Domain ↑ Ports ↑ Infrastructure`）推进到 P4/P5/P7 完成、
P3 收口；`api→store` / `application→store` 收敛；`StoreBackend` 拆除。

**边界（本线不做，列 §7 相邻积压）**：API key 相关（重灌 feed/embedding backfill）、`.ok()` 硬化、
cron vars、历史资源清理、intelligence-domain 裁决、sprint 编号文档归一。

**不变量（贯穿每一 commit）**：测试数不减反增；`GRANDFATHERED` 保持空表；每次迁移满足
before tests → migrate → after tests；fmt/clippy/wasm 门禁绿。

---

## 2. Phase A — P3 收尾（wire `D1DecisionRepository`）

参考 p2-port-closure "Residual P2 port state"：DecisionRepository adapter
`D1DecisionRepository` 排期 Phase 6.2C，**未接线**。

**Recon（动代码前必做，产出为准，勿按本文件猜）**
1. 读 `crates/infrastructure/src/decision_repository.rs` —— 是完整 adapter 还是 stub？trait 面
   `DecisionRepository` 定义在 `crates/decision-engine/src/repository.rs`，比对两方签名。
2. 找 decision-engine 的真实消费方：grep `decision_engine::` / 决策用例 在 `api`、`worker-entry`，
   确认它们当前是直接 `store`（`store/src/backend.rs` decision 段 / `d1_delegate.rs`）还是已走端口。
3. 顺带核对 signal 侧无残留直连 store（P3 R2 已 port，快速 grep 确认即可）。

**Actions**
- 组装点注入：在 `worker-entry` composition root 实例化 `D1DecisionRepository`（依赖 `D1Store`），
  传入 decision 消费者，替换其 `StoreBackend`/`store` 直连。
- 若 adapter 缺方法 → 补齐并加 T2 式映射测试（entity↔row 双向）。
- 不动 claim/observation/framework（有意不建 adapter）。

**Verify / Commit**
- `bash scripts/check-layered-deps.sh` → 空表；decision 消费方 `use store::` 归零（若涉及）。
- `cargo test --workspace`（计数 ≥ before）；fmt/clippy/wasm 绿。
- Commit：`refactor(decision-engine): wire D1DecisionRepository through composition root`。

---

## 3. Phase B — P4 收缩/拆除 `StoreBackend`

最终 `StoreBackend = 0`（final-arch DoD）。现 `store/src/backend.rs` supertrait（约 50 方法，
`#[deprecated]`）+ `d1_delegate.rs`（~774 行转发）+ `memory/backend.rs`（~1328 行）。

**Recon**
1. 按 bounded context 盘点 supertrait 方法 → 每个已被哪个 infra adapter 覆盖（signal/context/
   reflection/memory/decision 已 port，剩 article/feed/briefing/entity/observation/claim 段）。
2. **关键风险**：`MemoryStore`（memory/backend.rs）是测试替身。确认哪些测试/用例仍依赖它——
   在删之前要么迁移到 infra 的 in-memory adapter，要么按用例替换，不得裸删后全红。

**Actions（分 commit，逐个 bounded context）**
1. 未 port 的域方法：先给对应域在 infra 建 adapter（仅在**有真实消费方**时；无消费方的方法直接删，
   不建 speculative port —— 沿用 p2 的 SignalEvidence 门）。
2. 消费方全部切走后，删除 supertrait 方法 → 最终删 `backend.rs` supertrait 定义。
3. 删除 `d1_delegate.rs` 转发（被 store 的细粒度 `traits/*` 取代）。
4. MemoryStore：若仍作测试替身 → 保留在 store（作为 test util）或等价替换，并在 DoD 说明去处。

**Verify / Commit**
- 每步 `cargo test --workspace` 计数不降；`-D warnings`。
- Commit 粒度：每 bounded context 一个 `refactor(store): drop StoreBackend <domain> methods`。

---

## 4. Phase C — P5 application 唯一用例入口

**Actions**
- 把 `api` handler 里的业务编排上收为 `crates/application/src/services/*` 用例（decision/signal/
  briefing 等已有 `radar.rs`/`semantic_search.rs`/`decision.rs` 先例 → 按需扩展）。
- `api`/`worker-entry` 改为调用 application 用例；`api → store` 引用收敛到 0（DoD #3）。
- 依赖面收敛：api 只依赖 application + domain 类型 + worker。

**Verify / Commit**
- `grep -R "use store::" crates/api` = 0；`use application::` 覆盖所有用例路径。
- 架构护栏（Phase E）红线先行对齐。
- Commit：`refactor(api): route use-cases through application services (P5)`。

---

## 5. Phase D — P7 cargo-metadata 架构护栏（与 P5 并行可先行）

**Actions**
- 建 `crates/shared-kernel/tests/architecture.rs`（或独立 guard crate，不引新第三方依赖——
  decoupling plan Task 7.1 明示用 `cargo metadata` JSON）。
- 断言 forbidden edges：domain→store/worker；application→worker；api→infrastructure/vectorize/
  embedding/event-store/object-store；循环依赖。红线按 DoD。
- 接入 `.github/workflows/lint.yml`（独立 job）。

**Verify / Commit**
- CI job 独立跑，回归即红。Commit：`test(governance): architecture dependency guard (P7)`。

---

## 6. Phase E — P6（GATED）+ 收尾文档同步

**P6 — 等 intelligence-domain 裁决**（见 §0；裁决后再定 Phase 6 范围）。裁决前**不执行**删除
signal/claim/reflection/memory engine 壳、不删除 intelligence-domain、不搬 confidence。

**收尾（去耦主线结束时做，Task 7.2 基线）**
- 更新 `CLAUDE.md` 架构/依赖一节（依最终依赖图）。
- 更新 `FULL_REVIEW_REPORT.md` §5 基线（Health Score / 依赖图 / tech debt）。
- 归一 sprint 编号叙事（frozen 文档 Sprint 1–5 与执行中的 Sprint 6.5/6.2C 双轨）——作为独立 docs commit。

---

## 7. 相邻积压（tracked，不在本线内推进；均已在别处记录）

| 项 | 触发/依赖 | 记录处 |
|---|---|---|
| 重灌 feed/文章 + embedding backfill | 需 DeepSeek chat API key（用户约束：先不调 key） | `docs/ops/cf-reset-2026-09-03/CHAT_API_KEY_PENDING.md` + audit addendum |
| `.ok()` binding 静默失败硬化（R2/Vectorize/KV） | 独立加固 | audit Recommendations #1 |
| `CRON_REFLECTION_ENABLED`/`CRON_MEMORY_ENABLED` vars | 特性激活时 | audit #4 |
| KV `title` 自文档字段 | 小 | audit #2 |
| 历史资源清理（rss-*/portal-*/agent/index/…） | 独立授权任务 | audit Historical Resources |
| intelligence-domain 存续裁决 | 架构决定 | final-arch §4 标注 + 本 plan §0 |
| Vectorize 惯用法目标态 / 消息 `mark_failed` 语义 / outbox 迁移 | 架构/行为 | audit #3 / p2 行为注 |

---

## 8. 完成定义（本线阶段性）

- P3 收尾：`D1DecisionRepository` wired；决策消费方无 `store` 直连；测试不降。
- P4：`StoreBackend` supertrait 删除；`d1_delegate` 转发删除；MemoryStore 去处明确；测试不降。
- P5：`api → store = 0`；application 覆盖所有用例。
- P7：cargo-metadata 架构守卫入 CI 且绿。
- 全程：`GRANDFATHERED` 空表不变；fmt/clippy/wasm 绿。
- P6：仅在 intelligence-domain 裁决后按裁决范围执行。

## 9. Gates

- 每 Phase = 独立 commit。**push = BLOCKED，须用户明确确认**（沿用记忆 `sulix-cf-resource-policy`）。
- 本线纯代码，不触碰 Cloudflare allowlist/denylist 资源；`wrangler.toml` 不改。
