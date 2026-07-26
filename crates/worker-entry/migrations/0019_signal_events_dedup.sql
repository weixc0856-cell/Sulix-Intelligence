-- Signal Events write-amplification fix.
--
-- Prevents the cron-driven Signal Engine from inserting duplicate
-- "score_changed" / "created" events for the same signal thread on
-- the same calendar day.  Before creating the index we purge any
-- duplicates that accumulated before this migration was applied.

-- 1. Remove daily duplicates — keep the earliest event per
--    (thread_id, event_type, day).
DELETE FROM signal_events WHERE id NOT IN (
    SELECT MIN(id) FROM signal_events
    GROUP BY thread_id, event_type, date(created_at, 'unixepoch')
);

-- 2. Prevent future duplicates at the schema level.
CREATE UNIQUE INDEX IF NOT EXISTS idx_signal_events_daily
ON signal_events(thread_id, event_type, date(created_at, 'unixepoch'));
