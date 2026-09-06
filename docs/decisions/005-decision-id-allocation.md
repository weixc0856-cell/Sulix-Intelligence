# ADR-005: Decision ID Allocation Stays Single-Writer `MAX(id)+1`

**Date:** 2026-09-06

## Context

Decision identities are `DEC-{id:06}`, and the numeric suffix **is** the
`decisions` row primary key, written explicitly from the aggregate id before
the row exists:

- allocation reads the next id with `SELECT COALESCE(MAX(id), 0) + 1`
  (`store/d1/decision/crud.rs`);
- the propose flow embeds `DEC-{id}` into the domain event **before** any save
  (`application/services/decision.rs`), so the event contract is tied to a
  pre-assigned, caller-supplied id;
- `D1DecisionRepository::save` persists with `INSERT … ON CONFLICT(id) DO
  UPDATE`.

The single-writer assumption is already documented on the domain port
(`domain/traits/decision_id_source.rs`: a documented risk), not an unspoken
one.

The 2026-09-06 decision-vertical correctness audit — ②,
`docs/audit/2026-09-06-decision-vertical-correctness-review.md` — confirmed the
race is real, and worse than a failure: two concurrent creates can compute the
same next id, and the second `ON CONFLICT(id) DO UPDATE` then **overwrites the
first decision's whole row** — silent loss, no error surfaced.

## Decision

Keep the single-writer `MAX(id)+1` allocation. The product is solo-operator;
writes are overwhelmingly cron/queue-driven, and concurrent HTTP decision
creates are low frequency and do not occur today.

Do **not** adopt DB `INTEGER PRIMARY KEY AUTOINCREMENT` in this round. It is not
a local fix: it requires reversing the flow (insert row → read back the rowid →
compose `DEC-{id}` → then propose), decoupling the aggregate's public
`DEC-{id}` identity from the row primary key, and a change to the
`DecisionCreated` / propose event contract. Structural refactor — deferred.

The create path is **hardened now** (2026-09-06) rather than waiting for the
race to bite, and it hardens *without touching allocation*:

- `domain::DecisionUpsertStore::try_insert_decision` /
  `decision_engine::DecisionRepository::save_new` insert with
  `INSERT … ON CONFLICT(id) DO NOTHING`; `changes() == 1` iff this call created
  the row, so a duplicate primary key becomes a `Ok(false)` refusal instead of a
  silent `DO UPDATE` overwrite.
- `ApplicationService::create` loops on that refusal: on `Ok(false)` it re-runs
  `next_decision_id()` and re-proposes with the fresh id (bounded retries), so a
  create that loses the id race gets a *new* id rather than silently clobbering
  the winner's row.

Read-back-after-save was considered and rejected as the detection mechanism: it
is unsound under concurrency — the loser's read-back can observe the row before
the winner's overwrite lands, so it cannot detect the loss. The atomic
conditional insert has no such window. The `save` (idempotent upsert) path is
unchanged: a second save of the *same* aggregate id is an in-place update by
design.

## Consequences

- Single-writer `MAX(id)+1` remains the allocation scheme; no schema change and
  no event-contract change.
- A create that races another create now **retries on a fresh id instead of
  silently overwriting** — the audit ② clobber is closed on the code path, while
  allocation stays single-writer for the (rare) re-allocate loop.
- Cost: an extra `save_new` port + delegate across store/memory/infra/application
  and two bounded retry hops per collision — negligible at decision-create
  frequency.
- Structural refactor (DB auto-increment + `DEC`/row-pk decoupling) remains
  recorded as the accepted future work.
- DB auto-increment + `DEC`/row-pk decoupling is recorded as the accepted
  future refactor — revisit the moment a second write driver (multi-operator,
  queue-driven decision create) appears.
- Outbox best-effort for the same vertical is a separate, complementary
  decision — see ADR-006.
