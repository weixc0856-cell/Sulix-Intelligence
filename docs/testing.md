# 测试与覆盖率策略 (Testing & Coverage)

> 配套文档：`docs/superpowers/plans/2026-08-21-testing-plan.md`（Sprint 6.5 测试计划）。
> 本文是覆盖率门禁的**现行规则**——改动阈值/集合时必须同步更新本文。

## 一、目标

架构去耦（Ports & Adapters）进行期间，让"每域迁移必有测试护航"成为硬约束：

1. **测试基线全绿**：`cargo test --workspace --all-features` 零失败。
2. **覆盖率门禁**：纯逻辑层 + 应用层的行覆盖率 ≥ 70%，CI 自动拦截回归。
3. **wasm 例外白名单**：D1 SQL / wasm-bindgen 入口无法在 host 覆盖，允许豁免并**记录在案**。

---

## 二、覆盖率门禁集合（14 crates）

门禁只统计**纯逻辑层 + 应用层** crate——它们能脱离宿主完整执行，
覆盖数据才有"行为正确性"意义。基础设施宿主适配层（`store`、`event-store`、
`vectorize` 等）大量依赖 D1 SQL / wasm 运行时，host 覆盖率是误导性数字，
**不放入门禁**（其正确性由 host 集成测试 + wasm 测试保证）。

| 层 | crates | 覆盖来源 |
|---|---|---|
| 共享内核 | `shared-kernel`, `events` | 契约测试（serde round-trip、DTO 形状、trait 可驱动性） |
| 领域逻辑 | `rules`, `search`, `entity`, `content-governance`, `decision-engine`, `reasoning-framework`, `intelligence-domain`, `claim-engine`, `signal-engine` | 纯逻辑单测（MemoryStore 驱动） |
| 应用层 | `application`, `model-runtime` | 用例编排 + 服务测试 |
| 基础设施适配器 | `infrastructure` | host 注入测试（decision_repository / artifact_registry / provenance / storage_policy） |

> 集合的选择理由：这 14 个 crate 是"域逻辑 + 编排 + 适配器纯逻辑"的完整集合，
> 一旦某域迁移改动，门禁立刻暴露该域测试缺失。

---

## 三、阈值

| 指标 | 当前值 (2026-08-21) | 门禁阈值 | 爬坡 |
|---|---|---|---|
| 门禁集合行覆盖率 | **73.84%** | `--fail-under-lines 70`（**CI 硬门禁**） | 每 sprint +5% |
| 门禁集合区域覆盖率 | 76.37% | —（参考） | — |
| 门禁集合函数覆盖率 | 69.67% | —（参考） | — |
| 整个 workspace 行覆盖率 | 33.53% | ≥ 50%（目标，尚未门禁） | 每 sprint +5% |

- **门槛起步宽松**（纯逻辑 70% / 整体 50%），**每 sprint 上调 5%**，推动补测而非一次到位。
- **CI 只对 14-crate 门禁集合设硬门禁（70%）**。整个 workspace 的 33.53%
  被 wasm-bound crate（store/vectorize/embedding/api/worker-entry 等）拉低，
  这些 crate 的 D1/HTTP 路径无法在 host 覆盖——故 `--workspace ≥ 50%` 是
  追踪目标而非当日门禁，随各域迁移逐步接近。
- 上调节奏由维护者决定，但每次上调必须同步修改 `.github/workflows/coverage.yml` 与本文。

---

## 四、wasm 例外白名单

以下文件在门禁集合内但行覆盖率为 **0%** —— 它们全部是 wasm-only 入口，
host `cargo test` 无法执行，由 `#[cfg(all(test, target_arch = "wasm32"))]`
测试在 wasm 侧覆盖。**不属于门禁失败**。

| 文件 | 行数 | 0% 原因 |
|---|---|---|
| `intelligence/signal-engine/src/lib.rs` | 137 | 弃用编排 `SignalEngine::run()`（Sprint 6.2D 标记 DEPRECATED），无 host 测试 |
| `intelligence/signal-engine/src/query/{detail,entity,mod,radar}.rs` | 442 | D1 查询适配器（wasm async trait 实现） |
| `intelligence/signal-engine/src/discovery/{converter,retrieval}.rs` | 47 | 发现适配器 |
| `model-runtime/src/{deepseek,factory,retry,schema,task}.rs` | 72 | wasm HTTP 客户端 + 配置结构 |
| `decision-engine/src/proposal.rs` | 14 | 建议转换器（经 `reconstruct` 间接覆盖不足） |
| `claim-engine/src/llm.rs` | 8 | wasm 网关 stub |
| `entity/src/models.rs` | 8 | 纯数据模型（经 serde 间接覆盖） |
| `application/src/graph.rs` | 160 | 知识图谱查询（16.25%）——**次优，需关注** |

**规则**：新增 0% 文件必须在此表登记原因。无法解释的 0% 文件 = 门禁失败。

---

## 五、本地运行

```bash
# 门禁集合覆盖率 + 阈值校验（CI 同款命令）
cargo llvm-cov -p shared-kernel -p rules -p search -p entity -p content-governance \
  -p decision-engine -p reasoning-framework -p intelligence-domain -p application \
  -p model-runtime -p claim-engine -p signal-engine -p infrastructure -p events \
  --summary-only --fail-under-lines 70 --all-features

# 整个 workspace 覆盖率（整体阈值参考）
cargo llvm-cov --workspace --summary-only --all-features

# 生成 lcov 报告（PR 上传用）
cargo llvm-cov -p <crate> --lcov --output-path lcov.info
```

前置：`cargo install cargo-llvm-cov` + `rustup component add llvm-tools-preview`。

---

## 六、CI 门禁

`.github/workflows/coverage.yml`（PR → master 触发，路径：`crates/**` 等）：

1. `dtolnay/rust-toolchain@stable` + `llvm-tools-preview`
2. `taiki-e/install-action@v2` 安装 `cargo-llvm-cov`
3. 门禁集合 `--summary-only --fail-under-lines 70 --all-features`
4. lcov 报告 `actions/upload-artifact@v4` 上传（PR 中可查看逐文件覆盖）

---

## 七、工作区整体覆盖率（参考基线）

`cargo llvm-cov --workspace --summary-only --all-features`（2026-08-21 实测）：

```
TOTAL   23810 regions (16074 missed, 32.49%)   2233 functions (1590 missed, 28.80%)   13723 lines (9122 missed, 33.53%)
```

**追踪目标**：每 sprint +5% 行覆盖率，向 50% 靠拢。主要拖累项是 wasm-bound
宿主适配层——它们需要在去耦各 Phase 中逐步获得 host 集成测试（T8）才能真正
起数，这也是为什么整体阈值暂不设 CI 硬门禁。
