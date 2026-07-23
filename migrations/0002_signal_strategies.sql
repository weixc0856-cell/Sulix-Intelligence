-- 0002_signal_strategies.sql
-- Adds metadata columns for the Signal Strategies product feature.
-- signal_type: aggregation dimension for the future Signal Dashboard
-- updated_at: tracks when a strategy was last modified

ALTER TABLE filter_rules ADD COLUMN signal_type TEXT;
ALTER TABLE filter_rules ADD COLUMN updated_at INTEGER DEFAULT 0;

-- Backfill updated_at for existing rows
UPDATE filter_rules SET updated_at = created_at WHERE updated_at = 0;
