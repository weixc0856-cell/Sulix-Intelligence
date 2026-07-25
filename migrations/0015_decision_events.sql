-- Decision Events — event-sourced timeline for Decision Loop.
--
-- Every state change in a decision's lifecycle is recorded as an event.
-- This enables future Memory Engine extraction without schema coupling.
--
-- Sprint 3.2: table creation only (no consumers yet)
-- Sprint 3.3+: event sourcing for Decision Evaluation

CREATE TABLE IF NOT EXISTS decision_events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    decision_id INTEGER NOT NULL,
    event_type  TEXT NOT NULL,       -- 'created' | 'outcome_added' | 'evaluation_added' | 'closed'
    payload     TEXT,                -- optional JSON context
    created_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_decision_events_decision ON decision_events(decision_id);
CREATE INDEX IF NOT EXISTS idx_decision_events_type ON decision_events(event_type);

-- Add evidence_url column to outcome_events (pure fact layer)
ALTER TABLE outcome_events ADD COLUMN evidence_url TEXT;
