-- Sprint 5.6: Observation lineage enrichment.
-- Adds source tracking fields to the observations table.
--
-- NOTE: the base `observations` table was never created by any migration in the
-- batch (store code INSERTs/SELECTs it since Observation Foundation), so a clean
-- fresh-database apply of 0001..0049 failed with "no such table: observations".
-- Base columns match the store write path (store/src/domain/observation/crud.rs);
-- url/article_id/registry_source_id are added by the ALTERs below. IF NOT EXISTS
-- makes this a no-op on any database that already has the table.

CREATE TABLE IF NOT EXISTS observations (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    source_type      TEXT NOT NULL,
    source_id        TEXT NOT NULL,
    title            TEXT NOT NULL,
    summary          TEXT NOT NULL DEFAULT '',
    content_hash     TEXT,
    observed_at      INTEGER NOT NULL,
    created_at       INTEGER NOT NULL DEFAULT (unixepoch())
);

ALTER TABLE observations ADD COLUMN url TEXT;
ALTER TABLE observations ADD COLUMN article_id INTEGER REFERENCES articles(id) ON DELETE SET NULL;
ALTER TABLE observations ADD COLUMN registry_source_id INTEGER REFERENCES sources(id) ON DELETE SET NULL;
