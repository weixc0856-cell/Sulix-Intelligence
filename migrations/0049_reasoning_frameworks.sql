-- Sprint 6.4: Reasoning Framework Engine
--
-- reasoning_frameworks: the registry of calibrated mental models
-- claim_reasoning_frameworks: which frameworks were applied to which claims
--   with confidence delta tracking for calibration

CREATE TABLE IF NOT EXISTS reasoning_frameworks (
    id                  TEXT PRIMARY KEY,                        -- "compound-growth"
    name                TEXT NOT NULL,                           -- "Compound Growth Analysis"
    category            TEXT NOT NULL,                           -- "financial_intelligence"
    description         TEXT NOT NULL,                           -- 2-3 sentence explanation
    trigger_rules       TEXT NOT NULL DEFAULT '[]',              -- JSON array of TriggerRule
    reasoning_template  TEXT NOT NULL DEFAULT '',                -- LLM prompt injection text
    evidence_requirements TEXT NOT NULL DEFAULT '[]',            -- JSON array of required evidence types
    calibration_score   REAL NOT NULL DEFAULT 0.0,               -- 0.0–1.0 historical accuracy
    usage_count         INTEGER NOT NULL DEFAULT 0,              -- times applied to claims
    confidence_delta_avg REAL NOT NULL DEFAULT 0.0,             -- avg confidence change when applied
    created_at          INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS claim_reasoning_frameworks (
    claim_id            INTEGER NOT NULL REFERENCES claims(id) ON DELETE CASCADE,
    framework_id        TEXT NOT NULL REFERENCES reasoning_frameworks(id),
    relevance           REAL NOT NULL DEFAULT 0.5,               -- 0.0–1.0 how relevant
    reasoning           TEXT,                                    -- why this framework applies
    confidence_before   REAL,                                    -- confidence without framework
    confidence_after    REAL,                                    -- confidence with framework applied
    PRIMARY KEY (claim_id, framework_id)
);

CREATE INDEX IF NOT EXISTS idx_crf_framework ON claim_reasoning_frameworks(framework_id);
CREATE INDEX IF NOT EXISTS idx_crf_claim ON claim_reasoning_frameworks(claim_id);
CREATE INDEX IF NOT EXISTS idx_rf_category ON reasoning_frameworks(category);
