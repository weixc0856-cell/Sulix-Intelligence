-- Signal Events — evolution history for signal threads.
--
-- Each event represents a state change or notable moment in a signal
-- thread's lifecycle. The timeline is built from these events rather
-- than raw intelligence_signals instances, giving a curated evolution view.
--
-- Event types: created, score_changed, status_changed, evidence_added,
--              entity_joined, accelerating, decaying, resolved

CREATE TABLE IF NOT EXISTS signal_events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    thread_id   INTEGER NOT NULL REFERENCES signal_threads(id) ON DELETE CASCADE,
    event_type  TEXT NOT NULL,
    payload     TEXT,
    created_at  INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_signal_events_thread ON signal_events(thread_id);
CREATE INDEX IF NOT EXISTS idx_signal_events_type ON signal_events(event_type);
