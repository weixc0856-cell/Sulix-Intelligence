-- Artifact Registry — unified metadata for all R2-stored Intelligence Assets.
--
-- Every R2 object that the pipeline writes (article snapshots, signal JSON,
-- brief reports, decision artifacts) gets a row here so that:
--   a) The pipeline can trace which version of which model produced it.
--   b) The frontend / admin tooling can list artifacts without scanning R2.
--   c) Future Pipeline Replay knows which artifacts to regenerate.
--
-- See also: Sulix 产品定位与战略（最终版）§8.2 — D1/R2 边界定义

CREATE TABLE IF NOT EXISTS artifact_registry (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    artifact_type    TEXT NOT NULL,            -- 'article_snapshot', 'signal', 'brief', 'decision'
    entity_id        INTEGER NOT NULL,         -- FK-like ref to owning entity (articles.id, etc.)
    r2_key           TEXT NOT NULL,
    schema_version   TEXT NOT NULL DEFAULT '1',
    model            TEXT,                     -- AI model name (NULL for raw snapshots)
    pipeline_version TEXT NOT NULL DEFAULT '0.1.0',
    metadata         TEXT,                     -- Extensible JSON blob
    created_at       INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_artifact_type ON artifact_registry(artifact_type);
CREATE INDEX IF NOT EXISTS idx_artifact_entity ON artifact_registry(entity_id);
