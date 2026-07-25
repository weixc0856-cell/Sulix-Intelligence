-- Object Outbox — guaranteed-delivery queue for R2 Memory Archive writes.
--
-- Every D1 state mutation that should also produce an R2 archive object
-- first writes an outbox row here (within the same D1 transaction or batch).
-- A cron-side archive worker then drains the outbox: reads pending rows,
-- writes the payload to R2, and marks the row as 'archived'.
--
-- This decouples the latency-sensitive D1 write path from the R2 write
-- path and ensures that an R2 failure never blocks the D1 transaction.
--
-- Design:
--   - object_key:      the R2 key (e.g. "memory/signals/42/events/1710000000.json")
--   - payload:         the JSON body to write (rendered at transaction time so
--                      it captures the exact state, not a stale snapshot)
--   - status:          'pending' → 'archived' | 'failed'
--   - retry_count:     incremented on each failed R2 write; archive worker
--                      caps retries at 3 before moving to 'failed'
--   - object_type:     discriminator for observability ("signal_event",
--                      "briefing", "decision_artifact", ...)

CREATE TABLE IF NOT EXISTS object_outbox (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    object_type    TEXT NOT NULL,
    object_key     TEXT NOT NULL,
    payload        TEXT NOT NULL,                           -- JSON
    status         TEXT NOT NULL DEFAULT 'pending',         -- pending | archived | failed
    created_at     INTEGER NOT NULL DEFAULT (unixepoch()),
    retry_count    INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_outbox_pending
ON object_outbox(status, created_at);
