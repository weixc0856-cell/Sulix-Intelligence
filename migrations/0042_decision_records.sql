-- Sprint 6.0: Decision Intelligence Foundation.
-- Decision records with claim linking.

CREATE TABLE decision_records (
    id                  INTEGER PRIMARY KEY,
    title               TEXT NOT NULL,
    context             TEXT,
    decision_type       TEXT,
    action              TEXT,           -- what will be done
    rationale           TEXT,
    confidence          REAL,
    status              TEXT NOT NULL DEFAULT 'proposed'
                        CHECK(status IN ('proposed','active','completed','superseded')),
    signal_id           INTEGER REFERENCES signal_threads(id),
    memo_json           TEXT,           -- saved Decision Memo artifact
    created_at          INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at          INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX idx_decision_records_status ON decision_records(status);
CREATE INDEX idx_decision_records_signal ON decision_records(signal_id);

-- Decision → Claim linking with rich relationship types
CREATE TABLE decision_record_claims (
    decision_id         INTEGER NOT NULL REFERENCES decision_records(id) ON DELETE CASCADE,
    claim_id            INTEGER NOT NULL REFERENCES claims(id),
    relationship        TEXT NOT NULL DEFAULT 'supports'
                        CHECK(relationship IN ('supports','contradicts','context','assumption')),
    PRIMARY KEY(decision_id, claim_id)
);
