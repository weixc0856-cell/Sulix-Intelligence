# ADR-006: Decision Event Outbox — Best-Effort by Decision, Replayable by Construction

**Date:** 2026-09-06

## Context

Decision write routes persist their rows through the application use-case, then
construct the outbox `EventEnvelope`s in the delivery adapter **after** the
use-case returns (SD-C, `docs/superpowers/plans/2026-09-06-decision-vertical.md`)
— `DecisionCreated` / `OutcomeObserved` / `DecisionStatusChanged` /
`DecisionEvaluated` stay deliberately outside the use-case's knowledge.

`emit_envelope` swallows the outbox insert error (`let _ = insert_outbox`):
best-effort, per SD-D. The failure mode is loss of the *event* only — the
decision / outcome / evaluation **rows** are already persisted before the
envelope is ever attempted.

The 2026-09-06 audit (④,
`docs/audit/2026-09-06-decision-vertical-correctness-review.md`) showed this is
**not a uniform system design**: reflection, memory and event-store writes all
propagate outbox errors (fail-closed); the decision route is the only emitter
that swallows them. SD-D also declines a transaction/unit-of-work abstraction:
the row write and the outbox insert are issued by different layers through
separate store calls, with no shared batch/transaction boundary today.

## Decision

Accept decision outbox best-effort as a **decision-specific policy**, justified
by what the other sources lack: every decision event is reconstructible from
fact rows.

| event | reconstructible from |
|---|---|
| `DecisionCreated` | `decisions` row |
| `OutcomeObserved` | `outcome_events` row |
| `DecisionStatusChanged` | `decisions` status bucket transitions (SD-A1 granularity) |
| `DecisionEvaluated` | `decision_evaluations` row |

A dropped event is therefore recoverable — that is what makes best-effort
acceptable *here* and only here. (A `DecisionStatusChanged` for an intermediate
lifecycle step may not be perfectly restored from the single current-bucket
column, but the row is authoritative for current status and reads come from
the row, so the gap is acceptable.)

The standing remedy is **reconciliation**, recorded as a follow-up (not this
round): a scheduled or manual job that rebuilds missing outbox / event-archive
rows for decisions from the fact tables, closing the silent-loss window.

Do **not**, this round: overturn SD-C/SD-D by moving envelope construction into
the use-case transaction; and do not extend best-effort to the sibling sources
— they stay fail-closed, because their events are not reconstructible the same
way.

## Consequences

- A decision outbox insert failure silently loses an *event* (never a row)
  today; read-side history can be incomplete until reconciliation runs.
- SD-C/SD-D layering is preserved; no code change this round.
- Follow-ups: the reconciliation job; optionally flipping the decision route to
  fail-closed (500 + retry) once decision events gain consumers that cannot
  tolerate loss — verify idempotent re-post (already-at-target status is a
  no-op) before adopting.
- Event-id uniqueness on this path is already fixed separately (③, commit
  `425ea53`), so a duplicate id can no longer drop a completion event from the
  queryable index independently of this policy.
