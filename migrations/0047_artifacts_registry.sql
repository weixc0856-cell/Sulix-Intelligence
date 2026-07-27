-- Sprint 6.2B: Artifact Registry — unified index for all AI-generated artifacts.
--
-- Every large object (decision memos, reflection results, reasoning traces,
-- claim analyses) is stored in R2 with a reference row here. Domain entities
-- reference artifacts by `id` instead of raw object keys, so storage backend
-- changes (R2 → S3 → IPFS) never leak into business logic.
--
-- This replaces the ad-hoc artifact_key / memo_json / result patterns from
-- earlier sprints. Old tables keep their TEXT columns during a dual-write
-- migration window; they will be dropped in a future sprint.

CREATE TABLE IF NOT EXISTS artifacts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    artifact_type   TEXT NOT NULL,      -- 'decision_memo', 'reflection_result', 'reasoning_trace', 'claim_analysis'
    owner_type      TEXT NOT NULL,      -- domain entity type: 'decision', 'reflection', 'claim'
    owner_id        TEXT NOT NULL,      -- domain entity ID: 'DEC-000001', 'REF-000001', 'CLM-000001'
    storage         TEXT NOT NULL DEFAULT 'r2',  -- 'r2', 's3', 'ipfs' (future)
    object_key      TEXT NOT NULL,      -- provider-native key (e.g. R2 path)
    content_type    TEXT NOT NULL DEFAULT 'application/json',
    content_hash    TEXT,               -- SHA-256 of content for dedup / integrity
    size_bytes      INTEGER NOT NULL DEFAULT 0,
    metadata        TEXT,               -- optional JSON metadata (model name, version, etc.)
    created_at      INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_artifacts_type_owner ON artifacts(owner_type, owner_id);
CREATE INDEX IF NOT EXISTS idx_artifacts_type ON artifacts(artifact_type);
