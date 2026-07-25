-- Missing indexes identified during query analysis (2026-07-24).
--
-- 1. articles(published_at DESC)
--    The existing idx_articles_feed_published (feed_id, published_at DESC)
--    is only effective when feed_id is in the WHERE clause. Cross-feed
--    listing queries — latest, trending, adjacent, by-tag, by-category,
--    article-trend — all ORDER BY published_at DESC without filtering
--    by feed_id, causing full table scans.
--
-- 2. feeds(status)
--    Every cron cycle queries WHERE status = 'active' to find feeds
--    due for fetch. With dozens of feeds this is fast, but at scale
--    (1000+ feeds) the scan becomes noticeable.
--
-- 3. feeds(category)
--    Used when filtering feeds by category in feeds_due_for_fetch
--    and in dashboard JOIN queries.

CREATE INDEX IF NOT EXISTS idx_articles_published ON articles(published_at DESC);
CREATE INDEX IF NOT EXISTS idx_feeds_status ON feeds(status);
CREATE INDEX IF NOT EXISTS idx_feeds_category ON feeds(category);
