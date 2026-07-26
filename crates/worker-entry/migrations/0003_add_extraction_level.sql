-- Add per-feed extraction policy so full-text fetching is opt-in per source,
-- not a default behavior applied to all feeds.  Default 'summary_only' means
-- the pipeline only uses whatever text the RSS/Atom entry already carries
-- (entry.summary / entry.content), without fetching the article's canonical
-- URL for full-text extraction.  Set to 'full_text' only for sources you've
-- manually evaluated and accepted the risk for (typically company blogs and
-- official announcements, not commercial news publishers).

ALTER TABLE feeds ADD COLUMN extraction_level TEXT NOT NULL DEFAULT 'summary_only';

-- Existing rows get the safe default; active feeds that are community or
-- company-blog sources can be opted in individually:
-- UPDATE feeds SET extraction_level = 'full_text' WHERE id = <id>;
