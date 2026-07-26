-- Sprint 5.4: Reflection Engine — Decision Learning Loop feedback node.
-- Stores reflection state + index. Full content lives in R2 artifacts.
-- See design spec: docs/superpowers/specs/2026-07-25-reflection-engine-design.md

CREATE TABLE IF NOT EXISTS reflections (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    decision_id         INTEGER NOT NULL,
    outcome_id          INTEGER,
    job_id              TEXT UNIQUE,
    status              TEXT NOT NULL DEFAULT 'pending',
    artifact_key        TEXT,
    result              TEXT,
    quality_score       REAL,
    generator_version   TEXT DEFAULT 'reflection-v1',
    lessons_count       INTEGER DEFAULT 0,
    rules_count         INTEGER DEFAULT 0,
    generated_by        TEXT DEFAULT 'system',
    retry_count         INTEGER DEFAULT 0,
    last_error          TEXT,
    started_at          INTEGER,
    lease_until         INTEGER,
    created_at          INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at          INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(decision_id)
);

CREATE INDEX IF NOT EXISTS idx_reflections_status ON reflections(status);
CREATE UNIQUE INDEX IF NOT EXISTS idx_reflections_job_id ON reflections(job_id);
