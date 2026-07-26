-- Sprint 5.9C: Claim Intelligence
-- Atomic, falsifiable judgments extracted from evidence.

CREATE TABLE claims (
    id                  INTEGER PRIMARY KEY,
    statement           TEXT NOT NULL UNIQUE,
    claim_type          TEXT NOT NULL DEFAULT 'fact'
                        CHECK(claim_type IN ('fact','trend','prediction','causal','opinion')),
    reasoning           TEXT,
    falsification       TEXT,
    status              TEXT NOT NULL DEFAULT 'active',
    article_id          INTEGER REFERENCES articles(id) ON DELETE SET NULL,
    observation_id      INTEGER REFERENCES observations(id) ON DELETE SET NULL,
    created_at          INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at          INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX idx_claims_status ON claims(status);
CREATE INDEX idx_claims_type ON claims(claim_type);
CREATE INDEX idx_claims_article ON claims(article_id);

-- Evidence linking claims to supporting articles
CREATE TABLE claim_evidence (
    claim_id            INTEGER NOT NULL REFERENCES claims(id) ON DELETE CASCADE,
    article_id          INTEGER NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
    relation            TEXT NOT NULL DEFAULT 'supports'
                        CHECK(relation IN ('supports','contradicts','weakens')),
    strength            REAL NOT NULL DEFAULT 1.0,
    created_at          INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY(claim_id, article_id)
);

CREATE INDEX idx_claim_evidence_article ON claim_evidence(article_id);
