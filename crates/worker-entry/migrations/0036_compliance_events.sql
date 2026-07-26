-- Sprint 5.6: Compliance audit event log (pre-schema, reserved).
-- Records all governance-related state transitions for audit trail.

CREATE TABLE compliance_events (
    id                  INTEGER PRIMARY KEY,
    event_type          TEXT NOT NULL
                        CHECK(event_type IN (
                            'ARTICLE_BLOCKED', 'SOURCE_VERIFIED', 'POLICY_CHANGED',
                            'TAKEDOWN_SUBMITTED', 'TAKEDOWN_APPROVED', 'TAKEDOWN_REJECTED',
                            'CONTENT_RESTORED', 'SOURCE_CREATED', 'SOURCE_UPDATED'
                        )),
    entity_type         TEXT NOT NULL,      -- 'source', 'article', 'takedown'
    entity_id           INTEGER NOT NULL,
    payload             TEXT,               -- JSON-encoded event details
    created_at          INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX idx_compliance_events_type ON compliance_events(event_type);
CREATE INDEX idx_compliance_events_entity ON compliance_events(entity_type, entity_id);
