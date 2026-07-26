-- Populate sources table from existing feeds.
-- Default: Tier2 (news media), SummaryAllowed, Unknown license.
-- Feeds with extraction_level = 'full_text' get FullTextAllowed.
INSERT OR IGNORE INTO sources (source_type, feed_id, name, tier, policy, license, attribution, verified)
SELECT
    'RssFeed',
    f.id,
    f.title,
    'Tier2',
    CASE
        WHEN f.extraction_level = 'full_text' THEN 'FullTextAllowed'
        ELSE 'SummaryAllowed'
    END,
    'Unknown',
    f.title,
    0
FROM feeds f;
