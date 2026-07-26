-- Sprint 5.10 Phase 0: D1 Quota Recovery — Step 2: Controlled Cleanup
-- Run 0028a first. Verify counts before DELETE executes.
-- These DELETEs are safe: they target development detritus, not production data.

-- 1. Count stale test articles (no R2 content, > 7 days old)
SELECT 'stale_articles' AS check_name, COUNT(*) AS cnt
FROM articles WHERE raw_content_r2_key IS NULL AND created_at < unixepoch() - 86400 * 7;

-- 2. Count old context snapshots (> 30 days)
SELECT 'stale_snapshots' AS check_name, COUNT(*) AS cnt
FROM context_snapshots WHERE created_at < unixepoch() - 2592000;

-- 3. Count processed outbox entries (> 1 day)
SELECT 'stale_outbox' AS check_name, COUNT(*) AS cnt
FROM object_outbox WHERE status = 'archived' AND created_at < unixepoch() - 86400;

-- 4. DELETE (verify counts above before uncommenting)
-- DELETE FROM articles WHERE raw_content_r2_key IS NULL AND created_at < unixepoch() - 86400 * 7;
-- DELETE FROM context_snapshots WHERE created_at < unixepoch() - 2592000;
-- DELETE FROM object_outbox WHERE status = 'archived' AND created_at < unixepoch() - 86400;
