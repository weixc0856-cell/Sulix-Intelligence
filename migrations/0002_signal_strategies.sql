-- 0002_signal_strategies.sql
-- Adds metadata columns for the Signal Strategies product feature.
-- signal_type: aggregation dimension for the Signal Dashboard
-- score_delta: separates the numeric weight from rule_json condition
-- updated_at: tracks when a strategy was last modified

ALTER TABLE filter_rules ADD COLUMN signal_type TEXT;
ALTER TABLE filter_rules ADD COLUMN score_delta REAL NOT NULL DEFAULT 0;
ALTER TABLE filter_rules ADD COLUMN updated_at INTEGER DEFAULT 0;

-- Backfill updated_at and score_delta for existing rows
UPDATE filter_rules SET updated_at = created_at WHERE updated_at = 0;
