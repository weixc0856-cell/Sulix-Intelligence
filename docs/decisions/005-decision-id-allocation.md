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

If and when concurrent decision creates become real, harden without touching
allocation: verify by reading the row back after save, or map a duplicate-key
write to `409` + client retry (idempotent re-posts already no-op).

## Consequences

- Single-writer remains a documented correctness assumption on decision create;
  a genuinely concurrent create can still silently clobber a row (probability
  low today).
- No schema change and no event-contract change.
- DB auto-increment + `DEC`/row-pk decoupling is recorded as the accepted
  future refactor — revisit the moment a second write driver (multi-operator,
  queue-driven decision create) appears.
- Outbox best-effort for the same vertical is a separate, complementary
  decision — see ADR-006.
