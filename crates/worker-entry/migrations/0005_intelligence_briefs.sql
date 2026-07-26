-- Daily Intelligence Brief history table.
-- Stores generated briefing JSON for historical reference and trend analysis.
-- The KV cache handles hot reads; D1 is the source of truth.

CREATE TABLE IF NOT EXISTS intelligence_briefs (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    date          TEXT NOT NULL UNIQUE,        -- YYYY-MM-DD
    generated_at  INTEGER NOT NULL,           -- unix timestamp
    signal_count  INTEGER NOT NULL DEFAULT 0, -- number of signals analyzed
    content       TEXT NOT NULL,               -- JSON: Briefing struct
    created_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_intelligence_briefs_date ON intelligence_briefs(date);
