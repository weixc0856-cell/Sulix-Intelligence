-- Sprint 6.1: Artifact Registry — unified reference layer between D1 and R2.
-- Business tables reference artifacts.id instead of storing raw R2 keys.

CREATE TABLE artifacts (
    id                  INTEGER PRIMARY KEY,
    artifact_type       TEXT NOT NULL,          -- "decision_memo" | "reasoning_trace" | "reflection" | "evaluation"
    artifact_key        TEXT NOT NULL UNIQUE,   -- r2://sulix-prod/artifacts/...
    content_type        TEXT NOT NULL,          -- "application/json" | "text/markdown"
    size_bytes          INTEGER,
    hash                TEXT,                   -- sha256 for integrity verification
    version             INTEGER NOT NULL DEFAULT 1,
    metadata            TEXT,                   -- JSON-encoded metadata
    created_at          INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX idx_artifacts_type ON artifacts(artifact_type);
