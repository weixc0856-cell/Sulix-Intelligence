-- Signal Threads — long-lived intelligence signal assets.
--
-- Signal = Thread (long-term trend) + Instance (daily detection snapshot).
-- Decision Loop references signal_thread_id, not individual instances.
--
-- signal_key format: "entity:{entity_id}" (entity-anchored signals)
-- Future: "theme:{cluster_id}" for semantic theme signals (V2)

CREATE TABLE IF NOT EXISTS signal_threads (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    signal_key       TEXT NOT NULL UNIQUE,
    anchor_entity_id INTEGER REFERENCES entities(id),
    title            TEXT NOT NULL,
    description      TEXT NOT NULL DEFAULT '',
    status           TEXT NOT NULL DEFAULT 'active',
    health_score     REAL NOT NULL DEFAULT 0.0,
    first_seen_at    INTEGER,
    last_seen_at     INTEGER,
    created_at       INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at       INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_signal_threads_key ON signal_threads(signal_key);
CREATE INDEX IF NOT EXISTS idx_signal_threads_status ON signal_threads(status);
CREATE INDEX IF NOT EXISTS idx_signal_threads_entity ON signal_threads(anchor_entity_id);
