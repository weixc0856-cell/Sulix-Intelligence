-- Sprint 5.3: Decision Loop → Memory Event Stream
--
-- Changes:
--   1. Deprecate orphaned decision_events table (replaced by EventStore + event_archive_index).
--   2. Add columns to outcome_events matching Rust code expectations.
--   3. Align event_archive_index.aggregate_id type for string IDs ("DEC-000123").

-- decision_events had zero consumers since creation. Replaced by EventStore R2 archive.
ALTER TABLE decision_events RENAME TO legacy_decision_events;

-- outcome_events: add columns that the Rust code queries.
ALTER TABLE outcome_events ADD COLUMN evidence_url TEXT;
ALTER TABLE outcome_events ADD COLUMN observed_at INTEGER;

-- event_archive_index: ensure TEXT aggregate_id column accepts string IDs.
-- SQLite is lenient here, but we add an index hint for the string pattern.
CREATE INDEX IF NOT EXISTS idx_event_archive_aggregate_str
ON event_archive_index(aggregate_type, aggregate_id);
