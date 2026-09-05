# Sulix Intelligence — 最终前后端架构整改方案（冻结 v2）

> **状态：FROZEN（冻结）— 2026-08-21**
> 本文档为最终架构权威版本。后续所有前端/后端迁移、测试、Contract 工作均以此为准。
> 关联：前端执行计划 `docs/superpowers/plans/2026-08-21-architecture-decoupling-plan.md` + `2026-08-21-testing-plan.md`；前端 `d:/Project/intel-web/docs/architecture/final-architecture-v2.md`（同源镜像）。

---

## 0. 核心原则

> **前后端统一 Bounded Context 与 Contract，架构模式不强求完全同构；后端采用严格 Ports & Adapters，前端采用 Vertical Slice DDD；以测试和依赖边界作为最终架构护栏。**

---

## 1. 最终目标

当前系统处于"新旧架构并存"的迁移阶段。最终必须消灭：

```text
Frontend
src/lib/api/*
src/lib/agent/*

Backend
StoreBackend
api → store
application → store
intelligence-domain（伪迁移）
```

---

## 2. 前后端"领域对齐"，但"不强制结构同构"

前后端共享 **Bounded Context / API Contract / Domain Vocabulary / DTO semantics / Error semantics**，而不是 `Rust crate == TypeScript folder`。

| Backend           | Frontend     |
| ----------------- | ------------ |
| article           | articles     |
| feed              | feeds        |
| system            | system       |
| rules/strategies  | strategies   |
| agent             | agent        |
| briefing/entities | intelligence |
| decision loop     | decision     |

---

## 3. Backend 最终架构

严格分层：`Delivery → Application → Domain ↑ Ports ↑ Infrastructure`。

- **Domain**：Entity / VO / Aggregate / Domain Service / Rule。禁止 store/vectorize/embedding/D1/R2/HTTP/Cloudflare。
- **Application**：Use Case / Orchestration / Transaction / Port invocation。不 `store.xxx()`，而调 `DecisionRepository` 等 Port。
- **Ports**：`DecisionRepository`、`SignalRepository`、`ObservationRepository`、`ClaimRepository`、`ReflectionRepository`、`MemoryRepository`；未来 `EmbeddingPort`、`VectorSearchPort`、`ArticleRepository`、`FeedRepository`。
- **Infrastructure**：`D1DecisionRepository`、`D1SignalRepository`、`D1ObservationRepository`、`D1MemoryRepository`、`CloudflareVectorize`、`CloudflareR2`、`EmbeddingProvider`。**每个 adapter 必须拥有自己的映射测试。**

---

## 4. Backend 去耦执行顺序（P0–P7）

- **P0 — Baseline**：fmt、clippy、test、dependency check，先恢复绿色基线。
- **P1 — Dependency Fence**：cargo-deny / metadata 禁止 domain/application/api 依赖 store/vectorize/embedding。
- **P2 — Ports**：建立 Decision/Signal/Observation/Claim/Reflection/Memory Repository。
- **P3 — Adapter Migration**：Store 实现拆到 infrastructure/，每个 adapter 加测试。
- **P4 — Remove StoreBackend**：按 bounded context 逐个迁移，`StoreBackend = 0`。
- **P5 — Application Extraction**：把 API handler 的业务编排抽成 Application Use Case。
- **P6 — Remove intelligence-domain**：彻底删除伪迁移层。
- **P7 — Architecture Guard**：CI 自动验证 domain→infrastructure / application→concrete adapter / api→store / StoreBackend 均 forbidden。

> ⚠️ **CONTRADICTION / 待决议（2026-09-05）**：本行 P6 "删除 intelligence-domain" 与下列冲突：
> (a) decoupling plan（`docs/superpowers/plans/2026-08-21-architecture-decoupling-plan.md` 目标态图 +
> Task 6.1）把 `intelligence-domain` 当作 signal/claim 领域类型的**永久归宿**（DoD #4 要求
> "`intelligence_domain::` 为唯一来源"）；(b) README 目标架构同样保留它；(c) Task B P6（commit
> `acfaff8`）刚把 confidence 纯逻辑**迁入** intelligence-domain。
> **裁决暂缓（2026-09-05 决定）**：去耦先推不受影响的 P3 收尾 / P4 / P5 / P7；P6 范围（删
> intelligence-domain 与否）等本矛盾裁决后再定。裁决后需同步更新本节、decoupling plan、README。
> 若保留：P6 语义改为"删除旧 engine 壳 + `store::domain/*` 伪迁移层"，不含 intelligence-domain。

---

## 5–10. Frontend（摘要，详见前端镜像文档）

- Vertical Slice DDD，`src/domains/<domain>/{application,domain,infrastructure,presentation,components}`。
- 层职责锁死：Domain 禁 HTTP/Astro/DOM；Application 承担页面用例；Infrastructure 管 HTTP/DTO/错误；Presentation 做 ViewModel/格式化/CSS class；Components 只接受 `data: XxxViewModel`。
- HTTP 统一为 `src/infrastructure/api/client.ts`（ApiClient/ApiError/NetworkError），删除 apiFetch。
- Articles 为第一条 Reference Implementation：`Article DTO → ArticlesApi → Query → Article Domain → Presenter → ArticleCardViewModel → ArticleCard → Page`。
- ArticleCard 组件是 Presentation consumer（`ArticleCard → ArticleCardViewModel`），不是 Domain consumer。

---

## 11–12. Contract 成为第三条主轴

`Frontend ↕ Contract ↕ Backend`。第一阶段不过度工程化：只做 `Backend DTO → Explicit API Contract → Frontend DTO`，不搞 OpenAPI/codegen/shared package。重点解决 null vs []、404、204、enum string、snake_case、date format、pagination、error response。

---

## 13–14. Testing 最终架构

**Backend 六层**：Domain unit、Application use case、Infrastructure adapter mapping、Cross-domain integration、Delivery、Production smoke。infrastructure 不能 0 测试。

**Frontend**：Domain unit、Presenter unit、Application query、API adapter、Page integration、Build/smoke。Domain ≥ 70%。

**硬约束：测试数量不能因为架构迁移而下降。** 每次 migration（P3/P4/P5）满足 before tests → migration → after tests。

---

## 15–16. 架构护栏

**Codegraph（Frontend）**：`domain ✕ infra/pages/components/lib`；`application ✕ pages/components/lib`；`infrastructure ✕ pages/components/other domains/lib`；`domains ✕ lib`；`pages ✕ lib`。尤其 `domains/** → lib/**` 必须为 0。

**cargo metadata（Backend）**：`domain ✕ infrastructure`；`application ✕ concrete infra`；`api ✕ store`；`domain/application ✕ StoreBackend`。全部 CI 自动检查。

---

## 17. 最终执行路线

```text
PHASE 0 Baseline Green → Backend(T1→T2, P1→P2) / Frontend(Phase 0) → P3 Adapter / Articles → P4 Store / Pages → P5 Application / delete lib/api → Contract → Intelligence/Decision → Remaining Domains → Architecture Guard → Final CI
```

- **Sprint 0**：后端 fmt/clippy/unused-deps/baseline tests；前端 tsc/build/test/graph baseline。
- **Sprint 1**：后端 T2 infra tests、P1 dependency fence、P2 ports；前端 articles domain + ArticleCard + trending/search/tags/categories。
- **Sprint 2**：后端 P3 adapter migration；前端 article detail + delete lib/api/articles + feeds。
- **Sprint 3**：后端 P4 StoreBackend removal；前端 intelligence + decision。
- **Sprint 4**：后端 P5 + P6；前端 system + strategies + agent + dashboard。
- **Sprint 5**：后端 P7 CI；前端 lib/api=0、lib/agent=0、codegraph=0、contract verification。

---

## 18. 最终 DoD

- **Backend**：StoreBackend=0、api→store=0、application→store=0、domain→infrastructure=0、intelligence-domain=0、每 adapter 有 mapping tests、clippy=0、fmt clean、tests 全绿、dependency architecture CI green。
- **Frontend**：lib/api=0、lib/agent=0、apiFetch=0、domain 不依赖 infrastructure、application 不依赖 pages/lib、components 只接受 ViewModel、页面不直接请求 API、graph violations=0、tsc=0、build green、vitest green。
- **Contract**：endpoint/DTO/nullable/enum/error/404/204/pagination semantics 全部明确。

---

## 19. 最终架构图

```text
                         SULIX INTELLIGENCE
                                │
                     ┌──────────┴──────────┐
                     │       CONTRACT      │
                     │ API / DTO / Schema  │
                     └──────────┬──────────┘
                                │
          ┌─────────────────────┴─────────────────────┐
          │                                           │
          ▼                                           ▼
 ┌────────────────────┐                    ┌────────────────────┐
 │    ASTRO FRONTEND  │                    │    RUST BACKEND    │
 │ Pages→Application→ │                    │ Delivery→Application│
 │ Domain→Presentation│                    │ →Domain↑Ports↑Infra │
 │ →Components→Infra  │                    │ D1/R2/Vectorize/Ext│
 │ →ApiClient         │                    └──────────┬─────────┘
 └─────────┬──────────┘                               │
           └──────────────────┬───────────────────────┘
                              ▼
                       Cloudflare Platform
```

---

## 20. 结论

**冻结：Backend = DDD + Ports & Adapters + Application Use Cases；Frontend = Vertical Slice DDD + Application + Presentation + Infrastructure；Shared = Contract-first API boundary；Persistence = Infrastructure only；Testing = Architecture migration 与测试迁移绑定；Enforcement = cargo metadata + codegraph + CI。**

前端现有 Phase 0–9 保留作为执行计划；后端 P1–P7 保留（详见 `docs/superpowers/plans/2026-08-21-architecture-decoupling-plan.md`）。新增唯一横向 Track = **Contract**，并将 **Articles** 定义为前端第一条 Reference Vertical Slice。

最终形成：**Bounded Context → Contract → Backend Domain / Frontend Experience Domain → Infrastructure → Cloudflare**。
