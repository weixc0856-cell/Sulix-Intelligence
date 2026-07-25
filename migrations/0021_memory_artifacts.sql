-- Memory Artifacts — unified metadata index for the R2 Intelligence Memory Archive.
--
-- Every immutable object stored in the R2 Memory Archive (briefings, decision
-- records, reflections, reports, ...) gets a row here so callers can list,
-- query, and verify artifacts without scanning R2.
--
-- R2  = canonical content (immutable, versioned JSON blobs)
-- D1  = metadata index (queryable, fast)
-- KV  = hot cache (24h TTL for daily artifacts)
--
-- Write order: R2 → D1 index → legacy mirror → outbox (recovery)
-- Read order:  KV → R2 → D1 legacy fallback

CREATE TABLE IF NOT EXISTS memory_artifacts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    artifact_type   TEXT NOT NULL,              -- daily_briefing, decision_record, reflection, ...
    artifact_date   TEXT NOT NULL,              -- YYYY-MM-DD
    object_key      TEXT NOT NULL,              -- R2 object key
    schema_version  INTEGER NOT NULL DEFAULT 1,
    content_hash    TEXT,                       -- SHA-256 hex digest
    size_bytes      INTEGER,
    metadata        TEXT,                       -- JSON: signal_count, insight_count, etc.
    created_at      INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(artifact_type, artifact_date)
);

CREATE INDEX IF NOT EXISTS idx_memory_artifacts_type_date
ON memory_artifacts(artifact_type, artifact_date DESC);
