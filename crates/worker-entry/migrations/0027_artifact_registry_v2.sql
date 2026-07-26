-- Sprint 5.9: Unified Artifact Registry + Storage Boundary Compliance.
--
-- 1. Unified artifacts table replaces ad-hoc artifact tracking.
--    All R2 objects get a row here so GC and discovery see everything.
-- 2. context_snapshots gets R2 pointer columns so large JSON moves to R2.
--
-- See: docs/architecture/storage-boundary.md

CREATE TABLE IF NOT EXISTS artifacts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    artifact_type   TEXT NOT NULL,          -- briefing | reflection | context_snapshot | signal_event | llm_response
    owner_type      TEXT,                   -- decision | reflection | memory | context
    owner_id        TEXT,                   -- DEC-001 | REF-001 | MEM-001 | CTX-xxx
    r2_key          TEXT NOT NULL,
    size_bytes      INTEGER,
    schema_version  INTEGER DEFAULT 1,
    created_at      INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_artifacts_owner ON artifacts(owner_type, owner_id);
CREATE INDEX IF NOT EXISTS idx_artifacts_type ON artifacts(artifact_type);

ALTER TABLE context_snapshots ADD COLUMN object_key TEXT;
ALTER TABLE context_snapshots ADD COLUMN object_size INTEGER;
ALTER TABLE context_snapshots ADD COLUMN schema_version INTEGER DEFAULT 1;
