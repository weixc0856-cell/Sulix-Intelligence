-- Sprint 5.4B: Confidence Event Stream (append-only)
-- Tracks confidence changes for decisions, claims, and signals over time.

CREATE TABLE confidence_events (
    id                  INTEGER PRIMARY KEY,
    entity_type         TEXT NOT NULL,      -- "decision" | "signal" | "claim"
    entity_id           TEXT NOT NULL,
    previous_confidence REAL,               -- NULL on first event
    confidence          REAL NOT NULL,
    reason              TEXT,               -- human-readable why it changed
    trigger_event       TEXT,               -- event type that caused the change
    created_at          INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX idx_confidence_entity
    ON confidence_events(entity_type, entity_id, created_at);
