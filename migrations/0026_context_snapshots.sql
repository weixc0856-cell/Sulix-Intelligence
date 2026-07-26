CREATE TABLE IF NOT EXISTS context_snapshots (
    id              TEXT PRIMARY KEY,
    query           TEXT NOT NULL,
    intent          TEXT NOT NULL,
    domain          TEXT,
    engine_version  TEXT NOT NULL DEFAULT 'context-engine-v1',
    context_json    TEXT NOT NULL,
    evidence_refs   TEXT,
    confidence      REAL NOT NULL DEFAULT 0.0,
    user_scope      TEXT,
    created_at      INTEGER NOT NULL DEFAULT (unixepoch())
);
