-- Sprint 5.6: Observation lineage enrichment.
-- Adds source tracking fields to the observations table.

ALTER TABLE observations ADD COLUMN url TEXT;
ALTER TABLE observations ADD COLUMN article_id INTEGER REFERENCES articles(id) ON DELETE SET NULL;
ALTER TABLE observations ADD COLUMN registry_source_id INTEGER REFERENCES sources(id) ON DELETE SET NULL;
