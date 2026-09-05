# P2 收口 — Domain Port Closure (2026-09-03)

P2 (Ports) partial completion: Reflection + Memory bounded contexts decoupled onto
owned repository ports; the architecture-doc 6-repo port list is now fully
represented at the domain edge. See `final-architecture-v2.md` §4.

## What closed this round

### ReflectionEngine → `reflection-engine` owned port
- **Port** [`ReflectionRepository`](../../crates/intelligence/reflection-engine/src/repository.rs) —
  `create` / `update` / `find_latest_for_decision` / `load_decision_context` / `enqueue_event`.
- Domain records owned by the crate: `ReflectionUpdate`, `ReflectionRecord`, `DecisionFacts`
  (no `store` types cross the boundary). `ReflectionContextBuilder` + `ReflectionEngine`
  are now generic over the port, not `StoreBackend`.
- Adapter: `infrastructure::reflection_repository::D1ReflectionRepository`.
- Scheduling queries moved to the composition root (worker-entry holds its own `D1Store`).

### MemoryEngine → `memory-engine` owned port
- **Port** [`MemoryRepository`](../../crates/memory-engine/src/repository.rs) —
  `create_memory` / `enqueue_event` / `list_reflection_events`.
- Domain records owned by the crate: `NewMemory`, `MemoryEventRef`, `PromotionScore`
  (moved out of `store`; store's copy remains for other consumers).
- Adapter: `infrastructure::memory_repository::D1MemoryRepository`.

### Guard tightened
`scripts/check-layered-deps.sh` GRANDFATHERED 13 → 10 (removed
`reflection-engine:store`, `memory-engine:store`, `memory-engine:event-store` —
the last was declared but unused in code).

> 后续收口（2026-09-05）：P3-C1/C2（`d25e036`/`876ec25`）与 P6 置信度归域（`acfaff8`）续将
> GRANDFATHERED 10 → 8 → 7 → **0**。脚本现为空表；本 doc 属 P2 时点快照。详见
> `docs/superpowers/plans/2026-08-21-architecture-decoupling-plan.md`。

## SignalEvidence aggregate-ownership gate (no new port)

Signal evidence is held by the Signal aggregate: instances/evidence are appended
through the signal thread lifecycle (`SignalRepository`/signal instance rows), and
each instance carries its evidence summary + timeline. Evidence has **no
independent lifecycle** (no standalone persistence / cross-signal reuse / separate
query). Per the gate, the existing signal persistence suffices — **no
`SignalEvidenceRepository` is introduced** (avoiding a speculative port).

## Residual P2 port state (recorded, not changed this round)

| Port | Home | Adapter | Wired |
| --- | --- | --- | --- |
| DecisionRepository | decision-engine | `D1DecisionRepository` | scheduled Phase 6.2C |
| Signal/Observation/Claim | intelligence-domain | none | decorative; crate slated for P6 deletion |
| FrameworkRepository | reasoning-framework | none | no production consumer yet |
| ReflectionRepository | reflection-engine | `D1ReflectionRepository` | worker-entry + api route |
| MemoryRepository | memory-engine | `D1MemoryRepository` | worker-entry job |

## Behavioural notes

- `mark_failed` retry lookup is preserved verbatim (still resolves by the id passed
  in — a latent by-id semantics); fixing it is a separate behavioural change.
- Outbox writes (`enqueue_event`) stay on both ports as an explicit seam until the
  store-backend doc's scheduled relocation of the outbox to shared/events.
