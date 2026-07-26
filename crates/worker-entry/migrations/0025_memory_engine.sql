-- Sprint 5.5: Memory Engine — Cognitive Knowledge Layer.
-- Stores Belief Object metadata (lineage, origin, confidence decay fields).
-- Full content lives in R2 artifacts (memory/insights/MEM-{id}.json).
-- See design spec: docs/superpowers/specs/2026-07-26-memory-engine-design.md

CREATE TABLE IF NOT EXISTS memory_index (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    memory_type         TEXT NOT NULL,
    memory_origin       TEXT NOT NULL DEFAULT 'derived',
    statement           TEXT NOT NULL,
    confidence          REAL NOT NULL DEFAULT 0.0,
    stability_score     REAL,
    confidence_updated_at INTEGER,
    memory_sources      TEXT,
    artifact_key        TEXT,
    status              TEXT NOT NULL DEFAULT 'candidate',
    usage_count         INTEGER DEFAULT 0,
    validation_count    INTEGER DEFAULT 0,
    promoted_at         INTEGER NOT NULL DEFAULT (unixepoch()),
    deprecated_at       INTEGER,
    last_used_at        INTEGER,
    created_at          INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(artifact_key)
);

CREATE INDEX IF NOT EXISTS idx_memory_type ON memory_index(memory_type);
CREATE INDEX IF NOT EXISTS idx_memory_status ON memory_index(status);
CREATE INDEX IF NOT EXISTS idx_memory_origin ON memory_index(memory_origin);
