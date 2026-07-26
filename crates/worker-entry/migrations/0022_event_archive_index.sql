-- Event Archive Index — metadata index for the Memory Event Stream (R2).
--
-- Every event written via EventStore::append_event() gets a row here so
-- consumers can query by (aggregate_type, aggregate_id) without listing
-- R2 prefixes.
--
-- R2  = event payload (immutable JSON, partitioned by date)
-- D1  = index (fast queries by aggregate)
-- Outbox = ordering guarantee (outbox-first pattern)
--
-- Write order: D1 outbox → D1 index → archive worker → R2
-- Read order:  D1 index → R2 batch get → legacy D1 fallback

CREATE TABLE IF NOT EXISTS event_archive_index (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id        TEXT NOT NULL UNIQUE,
    aggregate_type  TEXT NOT NULL,          -- signal_thread, decision, outcome, ...
    aggregate_id    INTEGER NOT NULL,
    event_type      TEXT NOT NULL,          -- SignalScoreChanged, DecisionCreated, ...
    object_key      TEXT NOT NULL,          -- R2 object key
    occurred_at     INTEGER NOT NULL,
    created_at      INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_event_archive_aggregate
ON event_archive_index(aggregate_type, aggregate_id, occurred_at DESC);
