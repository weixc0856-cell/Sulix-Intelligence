-- Signal Discovery V2 — semantic clustering support.
--
-- Adds provenance tracking so we can distinguish entity-driven signals
-- from semantic-discovery signals (and later hybrids).

ALTER TABLE signal_threads ADD COLUMN discovery_method TEXT DEFAULT 'entity';
-- 'entity' | 'semantic' | 'hybrid'
