-- Sprint 6.0: Decision outcomes — multiple metrics per decision for calibration.

CREATE TABLE decision_outcomes (
    id                  INTEGER PRIMARY KEY,
    decision_id         INTEGER NOT NULL REFERENCES decision_records(id) ON DELETE CASCADE,
    metric              TEXT NOT NULL,          -- "users", "revenue", "retention"
    expected_value      TEXT,
    actual_value        TEXT,
    measurement_method  TEXT,
    status              TEXT NOT NULL DEFAULT 'pending'
                        CHECK(status IN ('pending','achieved','missed','superseded')),
    observed_at         INTEGER,
    created_at          INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX idx_outcomes_decision ON decision_outcomes(decision_id);
