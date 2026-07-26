# Worker Contract v1

**Status:** Frozen at Sprint 5.8

**Principle:** Workers are not intelligence engines. Workers are reliable async schedulers.

## Worker Responsibilities

Workers MAY:

- ✅ Read checkpoint state (KV, D1)
- ✅ Scan for pending work (eligible decisions, failed retries, stale leases)
- ✅ Invoke Engine/Service crates (`ReflectionEngine`, `MemoryPromotion`, etc.)
- ✅ Update status after completion (D1, KV, outbox)
- ✅ Write operational logs (`console_log!`)

Workers MUST NOT:

- ❌ Construct LLM prompts or call LLM directly
- ❌ Compute business scores or evaluate domain logic
- ❌ Modify domain rules or aggregate state directly
- ❌ Build or assemble event payloads (use Engine/Service for that)

## Worker Pattern (canonical)

```rust
// 1. Check guard / checkpoint
let last_run = kv.get(KV_LAST_RUN).await;
if recently_run { return; }

// 2. Scan for pending work
let pending = store.eligible_items().await;

// 3. Invoke engine (not inline logic)
for item in pending {
    engine.execute(item).await;
}

// 4. Persist checkpoint
kv.put(KV_LAST_RUN, now).await;
```

## Current Workers

| Worker | Trigger | Engine Called | Lines (core) |
|--------|---------|---------------|--------------|
| `reflection.rs` | Cron | `ReflectionEngine::execute()` | ~70 |
| `memory.rs` | Cron | `MemoryPromotion::promote()` | ~50 |
| `archive.rs` | Cron | — (R2 + index writes) | ~100 |
| `signal.rs` | Cron | `SignalEngine::run()` | ~90 |

## Rationale

This contract exists to prevent Worker scope creep. The most common failure mode in Serverless architectures is the "Worker that grew into a monolith" — a single file that starts as 50 lines of dispatch and becomes 500 lines of prompt-building, retry logic, and domain rules.

Sulix keeps cognition in Engine crates and execution in Workers. If a Worker needs more than 150 lines of logic, extract an Engine crate.
