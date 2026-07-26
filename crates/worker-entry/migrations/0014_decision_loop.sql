-- Decision Loop — Verifiable Decision Records for the Intelligence OS.
--
-- A Decision Record captures a judgment based on Signal Thread evidence:
-- what we believe (hypothesis), how confident we are (confidence), what
-- action we're taking (decision_type), and whether we were right (outcome).
--
-- Decision is NOT a task manager. It is an Intelligence asset that enables
-- Outcome → Memory → Better Signal Evaluation.
--
-- Sprint 3.1: decisions table (core)
-- Sprint 3.2: outcome_events table (verification)

CREATE TABLE IF NOT EXISTS decisions (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    signal_thread_id INTEGER,               -- nullable: decisions can start from any observation
    actor_id      INTEGER,                  -- reserved for future user system

    decision_type TEXT NOT NULL,             -- 'observe' | 'monitor' | 'investigate' | 'act' | 'ignore'
    title         TEXT NOT NULL,
    hypothesis    TEXT,                      -- "what I expect to happen"
    rationale     TEXT,                      -- "why I think so"
    confidence    REAL DEFAULT 0.5,          -- 0.0 to 1.0

    status        TEXT DEFAULT 'active',     -- 'active' | 'completed' | 'superseded'
    priority      TEXT DEFAULT 'medium',     -- 'high' | 'medium' | 'low'

    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_decisions_signal ON decisions(signal_thread_id);
CREATE INDEX IF NOT EXISTS idx_decisions_status ON decisions(status);
CREATE INDEX IF NOT EXISTS idx_decisions_actor ON decisions(actor_id);

-- Outcome Events: how did the decision turn out?
-- Sprint 3.2 will wire this into the UI.
CREATE TABLE IF NOT EXISTS outcome_events (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    decision_id   INTEGER NOT NULL,
    outcome_type  TEXT,                      -- 'observation' | 'confirmation' | 'correction'
    observation   TEXT,                      -- "what actually happened"
    result        TEXT,                      -- 'confirmed' | 'partially' | 'contradicted'
    accuracy      REAL,                      -- 0.0 to 1.0 self-assessment
    created_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_outcomes_decision ON outcome_events(decision_id);
