-- Sprint 5.8: Add factors_json column to confidence_events for storing
-- per-factor breakdown of confidence calculations.
ALTER TABLE confidence_events ADD COLUMN factors_json TEXT;
