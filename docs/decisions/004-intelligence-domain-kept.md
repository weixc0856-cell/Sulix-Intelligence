# ADR-004: 保留 intelligence-domain（P6 范围修订）

- **状态**：Accepted（2026-09-06）
- **关联**：[final-architecture-v2.md §4](../architecture/final-architecture-v2.md)、
  [decoupling plan（08-21）](../superpowers/plans/2026-08-21-architecture-decoupling-plan.md)、
  [decoupling-advance（09-05）](../superpowers/plans/2026-09-05-decoupling-advance.md)

## 背景

`final-architecture-v2.md` §4 的 **P6** 原写 "Remove intelligence-domain"。但三处证据与它冲突：

- decoupling plan 目标态图 + DoD #4：`intelligence_domain::` 为 signal/claim 领域类型的**唯一来源**。
- README 目标架构同样保留 `intelligence-domain`。
- Task B P6（`acfaff8`，2026-09-05）刚把 confidence 纯逻辑**迁入** intelligence-domain。

裁决自 2026-09-05 起暂缓（先推 P3 收尾 / P4 / P5 / P7 / 主线 C1–C7），至 2026-09-06 定案。

## 决策

**保留 `crates/intelligence-domain`**，定为 signal/claim/decision 纯领域类型与纯逻辑（含 confidence）的
永居所。相应地：

- **P6 收口语义修订**为："删除旧 engine 壳 + `store::domain/*` 伪迁移层"，**不含** intelligence-domain。
- 旧表述 "Remove intelligence-domain" 作废；frozen-arch §4、decoupling plan 的矛盾注已消解并指向本 ADR。

## 两个 "domain" crate 的区分（低优先消歧项）

| crate | 职责 | 性质 |
|---|---|---|
| `crates/domain`（09-06 新建） | persistence 端口 + DTO + `StoreError` | 契约层（机械搬迁壳，被 store/application 依赖） |
| `crates/intelligence-domain`（既有） | 领域类型 / 纯逻辑（confidence、signal/claim 等） | 领域层（被 engine crates 依赖） |

两者更名消歧（如 `domain` → `persistence-ports`/`store-contracts`）另议，低优先 —— C1–C7 刚落地，
重命名 churn 高、收益低，暂不做。

## 影响

- `intelligence-domain` 内暂存的 `#[allow(dead_code)]`（engine.rs、signal.rs）保持 —— 属领域补全 backlog，
  不再因 "Phase 6 删除 crate" 而遗留。
- P6 剩余工作 = 删除旧 engine 壳 + `store::domain/*` 伪迁移层（若仍有残留），与 D2 decision vertical 正交。
