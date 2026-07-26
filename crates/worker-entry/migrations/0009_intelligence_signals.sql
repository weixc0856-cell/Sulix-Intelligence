-- Intelligence Signals — first-class intelligence artifacts.
--
-- Signal 是 Intelligence 一等对象，不是 Entity 的排名结果。
-- 每条 Signal 有独立 identity，entity 只是 anchor。
-- Decision Loop 通过 signal_id 引用，不依赖 entity_id。
-- title/summary 由 Signal Engine 生成，LLM 只负责 why_it_matters/recommendation/impact。

CREATE TABLE IF NOT EXISTS intelligence_signals (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    anchor_entity_id INTEGER REFERENCES entities(id),
    title            TEXT NOT NULL,
    summary          TEXT NOT NULL DEFAULT '',
    confidence       REAL NOT NULL DEFAULT 0.0,
    impact           TEXT NOT NULL DEFAULT 'Medium',
    trend            TEXT NOT NULL DEFAULT 'stable',
    article_count    INTEGER NOT NULL DEFAULT 0,
    source_count     INTEGER NOT NULL DEFAULT 0,
    created_at       INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at       INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_intel_signals_confidence ON intelligence_signals(confidence DESC);
CREATE INDEX IF NOT EXISTS idx_intel_signals_entity ON intelligence_signals(anchor_entity_id);
CREATE INDEX IF NOT EXISTS idx_intel_signals_created ON intelligence_signals(created_at DESC);

-- Signal ↔ Evidence articles (many-to-many)
CREATE TABLE IF NOT EXISTS signal_evidence (
    signal_id        INTEGER NOT NULL REFERENCES intelligence_signals(id) ON DELETE CASCADE,
    article_id       INTEGER NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
    PRIMARY KEY (signal_id, article_id)
);

-- Signal ↔ Related entities
CREATE TABLE IF NOT EXISTS signal_entities (
    signal_id        INTEGER NOT NULL REFERENCES intelligence_signals(id) ON DELETE CASCADE,
    entity_id        INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    PRIMARY KEY (signal_id, entity_id)
);
