-- sulix-feed-workspace initial schema (Cloudflare D1 / SQLite)
--
-- Notes specific to D1 (verified 2026-07):
--   1. D1 does not support transactions. articles_fts is declared as an
--      "external content" FTS5 table (content=articles) and kept in sync
--      via triggers instead of app-level transactions, so a single INSERT/
--      UPDATE/DELETE statement on `articles` atomically updates the index.
--   2. D1 currently fails to export a database that contains FTS5 virtual
--      tables (wrangler d1 export --remote). If you ever need to export,
--      drop articles_fts first, export, then re-run the CREATE VIRTUAL
--      TABLE + trigger statements below to rebuild it.

CREATE TABLE feeds (
    id                  INTEGER PRIMARY KEY,
    url                 TEXT NOT NULL UNIQUE,
    title               TEXT,
    category            TEXT,
    fetch_interval_sec  INTEGER NOT NULL DEFAULT 3600,
    last_fetched_at     INTEGER,
    etag                TEXT,       -- from prior response, sent as If-None-Match
    last_modified       TEXT,       -- from prior response, sent as If-Modified-Since
    status              TEXT NOT NULL DEFAULT 'active',
    created_at          INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE articles (
    id                  INTEGER PRIMARY KEY,
    feed_id             INTEGER NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
    guid                TEXT NOT NULL,
    title               TEXT NOT NULL DEFAULT '',
    url                 TEXT,
    published_at        INTEGER,
    raw_content_r2_key  TEXT,       -- original HTML/body lives in R2, D1 stores the pointer
    ai_summary          TEXT NOT NULL DEFAULT '',
    ai_tags             TEXT,       -- JSON array, e.g. '["macro","auto"]'
    vector_id           TEXT,       -- id of the corresponding embedding in Vectorize
    score                REAL NOT NULL DEFAULT 0,
    created_at          INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(feed_id, guid)
);

CREATE INDEX idx_articles_feed_published ON articles(feed_id, published_at DESC);
CREATE INDEX idx_articles_score ON articles(score DESC);

-- External-content FTS5 index over title + ai_summary.
CREATE VIRTUAL TABLE articles_fts USING fts5(
    title,
    ai_summary,
    content = 'articles',
    content_rowid = 'id'
);

-- Triggers keep articles_fts in lockstep with articles on every write,
-- which substitutes for the transactional consistency D1 doesn't provide.
CREATE TRIGGER articles_ai AFTER INSERT ON articles BEGIN
    INSERT INTO articles_fts(rowid, title, ai_summary)
    VALUES (new.id, new.title, new.ai_summary);
END;

CREATE TRIGGER articles_ad AFTER DELETE ON articles BEGIN
    INSERT INTO articles_fts(articles_fts, rowid, title, ai_summary)
    VALUES ('delete', old.id, old.title, old.ai_summary);
END;

CREATE TRIGGER articles_au AFTER UPDATE ON articles BEGIN
    INSERT INTO articles_fts(articles_fts, rowid, title, ai_summary)
    VALUES ('delete', old.id, old.title, old.ai_summary);
    INSERT INTO articles_fts(rowid, title, ai_summary)
    VALUES (new.id, new.title, new.ai_summary);
END;

-- Filter/scoring rules. Pure logic, decoupled from storage backend so the
-- `rules` crate can be unit-tested without touching D1 at all.
CREATE TABLE filter_rules (
    id            INTEGER PRIMARY KEY,
    name          TEXT NOT NULL,
    rule_json     TEXT NOT NULL,      -- parsed/executed by the `rules` crate
    audience_tag  TEXT NOT NULL DEFAULT 'default',
    enabled       INTEGER NOT NULL DEFAULT 1,
    created_at    INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX idx_filter_rules_audience ON filter_rules(audience_tag, enabled);

-- Auth tables intentionally omitted for now (free-tier launch per current
-- plan). Add a `users` table + Axum auth middleware later without touching
-- the schema above.
