-- Sprint 6.1: Artifact Lineage — provenance chain for Decision Graph.
-- Links every intelligence artifact to its source chain.
-- Uses string IDs ("claim:123" not just "123") to avoid type collision.

CREATE TABLE artifact_lineage (
    id                  INTEGER PRIMARY KEY,
    from_artifact_type  TEXT NOT NULL,          -- "source" | "observation" | "claim" | "signal" | "decision"
    from_artifact_id    TEXT NOT NULL,          -- "claim:123" | "article:456"
    to_artifact_type    TEXT NOT NULL,
    to_artifact_id      TEXT NOT NULL,
    relationship        TEXT NOT NULL,          -- "derived_from" | "supported_by" | "triggered_by"
    created_at          INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX idx_lineage_from ON artifact_lineage(from_artifact_type, from_artifact_id);
CREATE INDEX idx_lineage_to ON artifact_lineage(to_artifact_type, to_artifact_id);
