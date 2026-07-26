-- Sprint 5.6: Source Registry & Governance Layer
-- Content provenance and compliance metadata for every information source.

CREATE TABLE sources (
    id                  INTEGER PRIMARY KEY,
    source_type         TEXT NOT NULL DEFAULT 'RssFeed' CHECK(source_type IN ('RssFeed', 'Api', 'Manual', 'UserUpload')),
    feed_id             INTEGER UNIQUE REFERENCES feeds(id) ON DELETE CASCADE,
    name                TEXT,
    tier                TEXT NOT NULL DEFAULT 'Tier2' CHECK(tier IN ('Tier0', 'Tier1', 'Tier2', 'Tier3')),
    policy              TEXT NOT NULL DEFAULT 'SummaryAllowed' CHECK(policy IN ('MetadataOnly', 'SummaryAllowed', 'FullTextAllowed', 'UserOwned')),
    license             TEXT NOT NULL DEFAULT 'Unknown',
    license_detail      TEXT,
    attribution         TEXT,
    trust_score         REAL,
    retention_days      INTEGER,
    verified            INTEGER NOT NULL DEFAULT 0,
    notes               TEXT,
    created_at          INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at          INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX idx_sources_feed_id ON sources(feed_id);
CREATE INDEX idx_sources_tier ON sources(tier);
CREATE INDEX idx_sources_policy ON sources(policy);
