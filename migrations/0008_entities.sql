-- Entity Graph — structured knowledge graph over extracted entities.
--
-- Moves entities from `articles.ai_tags` JSON blobs into first-class D1 tables,
-- enabling entity-centric queries, graph traversal, and relationship tracking.
--
-- Design decisions:
--   - `normalized_name UNIQUE` rather than raw `name UNIQUE`: "OpenAI" and "Open AI"
--     map to the same row through the canonicalizer.
--   - Directed relations (`source_entity_id` / `target_entity_id`) support asymmetric
--     semantics (e.g. "NVIDIA depends_on TSMC" ≠ reverse).
--   - `confidence` replaces generic `weight` — expresses how certain the relation is.
--   - `mentioned_together` is the auto-generated co-occurrence type; semantic relations
--     are separately curated.

CREATE TABLE IF NOT EXISTS entities (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT NOT NULL,               -- Display name (canonical form)
    normalized_name TEXT NOT NULL UNIQUE,        -- Lowercase, stripped, for dedup
    entity_type     TEXT NOT NULL DEFAULT 'unknown',  -- vulnerability, organization, product, unknown
    canonical_id    INTEGER REFERENCES entities(id),  -- Self-reference for aliases
    description     TEXT,
    metadata        TEXT,                        -- Extensible JSON blob
    created_at      INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at      INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type);
CREATE INDEX IF NOT EXISTS idx_entities_canonical ON entities(canonical_id);
CREATE INDEX IF NOT EXISTS idx_entities_normalized ON entities(normalized_name);

-- Many-to-many: articles → entities
CREATE TABLE IF NOT EXISTS article_entities (
    article_id      INTEGER NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
    entity_id       INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    relevance       REAL NOT NULL DEFAULT 1.0,   -- Relevance score 0.0–1.0
    context         TEXT,                        -- Surrounding text snippet
    created_at      INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (article_id, entity_id)
);

CREATE INDEX IF NOT EXISTS idx_article_entities_entity ON article_entities(entity_id);
CREATE INDEX IF NOT EXISTS idx_article_entities_article ON article_entities(article_id);

-- Entity-to-entity relations (knowledge graph edges)
CREATE TABLE IF NOT EXISTS entity_relations (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    source_entity_id  INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    target_entity_id  INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    relation_type     TEXT NOT NULL,             -- mentioned_together, competes_with, depends_on, acquired_by, part_of
    confidence        REAL NOT NULL DEFAULT 1.0, -- 0.0–1.0
    first_seen_at     INTEGER NOT NULL DEFAULT (unixepoch()),
    last_seen_at      INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(source_entity_id, target_entity_id, relation_type)
);

CREATE INDEX IF NOT EXISTS idx_entity_relations_source ON entity_relations(source_entity_id);
CREATE INDEX IF NOT EXISTS idx_entity_relations_target ON entity_relations(target_entity_id);
