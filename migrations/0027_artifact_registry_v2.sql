-- Sprint 5.9: Unified Artifact Registry + Storage Boundary Compliance.
--
-- 1. context_snapshots gets R2 pointer columns so large JSON moves to R2.
-- 2. NOTE: this file's original `artifacts` CREATE TABLE + indexes were
--    REMOVED. 0027's r2_key/owner_type schema was superseded by 0044
--    (artifact_key/content_type/hash/version — the schema store code in
--    store/src/domain/artifact.rs actually reads/writes); 0047 declared yet
--    another (owner_type/object_key) that code never adopted. All three
--    CREATE TABLE artifacts conflicted, so a clean fresh-database apply of
--    0001..0049 failed with "table artifacts already exists". 0044 is the
--    single creator now; this file only does the context_snapshots enrich.
--
-- See: docs/architecture/storage-boundary.md

ALTER TABLE context_snapshots ADD COLUMN object_key TEXT;
ALTER TABLE context_snapshots ADD COLUMN object_size INTEGER;
ALTER TABLE context_snapshots ADD COLUMN schema_version INTEGER DEFAULT 1;
