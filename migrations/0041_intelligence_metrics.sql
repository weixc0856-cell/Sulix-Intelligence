-- Sprint 5.9D: Intelligence Evaluation & Calibration Runtime.
-- Records every AI reasoning run, its quality evaluation, and confidence calibration.

-- 1. Every AI model invocation (who, what, how long, how many tokens)
CREATE TABLE reasoning_runs (
    id                  INTEGER PRIMARY KEY,
    reasoning_type      TEXT NOT NULL,          -- "summarization" | "claim_extraction" | "reflection" | "agent"
    model_provider      TEXT NOT NULL,
    model_name          TEXT NOT NULL,
    prompt_hash         TEXT NOT NULL,
    input_hash          TEXT NOT NULL,
    input_tokens        INTEGER,
    output_tokens       INTEGER,
    latency_ms          INTEGER,
    response_json       TEXT,
    prompt_version      TEXT,
    runtime_version     TEXT,
    success             INTEGER NOT NULL DEFAULT 1,
    error               TEXT,
    created_at          INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX idx_reasoning_runs_type ON reasoning_runs(reasoning_type, created_at);

-- 2. Quality evaluation of a reasoning run (accuracy, reasoning_quality, claim_quality)
CREATE TABLE reasoning_evaluations (
    id                  INTEGER PRIMARY KEY,
    reasoning_run_id    INTEGER NOT NULL REFERENCES reasoning_runs(id),
    evaluation_type     TEXT NOT NULL,          -- "accuracy" | "reasoning_quality" | "claim_quality"
    score               REAL NOT NULL,
    criteria_json       TEXT,
    reviewer_type       TEXT DEFAULT 'automatic', -- "automatic" | "human" | "outcome"
    created_at          INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX idx_reasoning_eval_run ON reasoning_evaluations(reasoning_run_id);

-- 3. Confidence calibration: predicted confidence vs actual outcome
CREATE TABLE confidence_calibrations (
    id                  INTEGER PRIMARY KEY,
    entity_type         TEXT NOT NULL,          -- "claim" | "decision"
    entity_id           INTEGER NOT NULL,
    predicted_confidence REAL NOT NULL,
    actual_outcome      REAL,                  -- 0.0 or 1.0 after outcome is known
    calibration_error   REAL,                  -- (predicted - actual)^2
    outcome_id          INTEGER,
    created_at          INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX idx_calibration_entity ON confidence_calibrations(entity_type, entity_id, created_at);
