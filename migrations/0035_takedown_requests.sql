-- Sprint 5.6: Takedown request workflow.
-- Independent state machine: does NOT directly modify source policy.

CREATE TABLE takedown_requests (
    id                  INTEGER PRIMARY KEY,
    source_id           INTEGER REFERENCES sources(id) ON DELETE SET NULL,
    article_id          INTEGER REFERENCES articles(id) ON DELETE SET NULL,
    requester_email     TEXT NOT NULL,
    reason              TEXT NOT NULL,
    status              TEXT NOT NULL DEFAULT 'submitted'
                        CHECK(status IN ('submitted', 'reviewing', 'approved', 'rejected', 'resolved')),
    notes               TEXT,
    created_at          INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at          INTEGER NOT NULL DEFAULT (unixepoch()),
    processed_at        INTEGER
);

CREATE INDEX idx_takedowns_source ON takedown_requests(source_id);
CREATE INDEX idx_takedowns_status ON takedown_requests(status);

-- Content visibility overrides: applied when takedown is approved.
-- Does not modify underlying data, only API visibility.
CREATE TABLE content_visibility_overrides (
    id                  INTEGER PRIMARY KEY,
    source_id           INTEGER REFERENCES sources(id) ON DELETE CASCADE,
    article_id          INTEGER REFERENCES articles(id) ON DELETE CASCADE,
    takedown_id         INTEGER REFERENCES takedown_requests(id) ON DELETE SET NULL,
    action              TEXT NOT NULL DEFAULT 'block_serve'
                        CHECK(action IN ('block_serve', 'block_storage', 'block_embedding')),
    active              INTEGER NOT NULL DEFAULT 1,
    created_at          INTEGER NOT NULL DEFAULT (unixepoch()),
    expires_at          INTEGER
);

CREATE INDEX idx_overrides_source ON content_visibility_overrides(source_id);
CREATE INDEX idx_overrides_article ON content_visibility_overrides(article_id);
