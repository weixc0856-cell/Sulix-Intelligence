-- Discovery Engine performance index.
-- Optimises the query in recent_embedded_articles():
--   SELECT ... FROM articles
--   WHERE vector_id IS NOT NULL AND published_at >= ?
--   ORDER BY published_at DESC LIMIT ?
--
-- The partial index avoids indexing articles without embeddings,
-- and the (published_at DESC, vector_id) ordering matches the query
-- exactly, enabling SQLite to use a range scan without sorting.

CREATE INDEX IF NOT EXISTS idx_articles_published_vector
ON articles(published_at DESC, vector_id)
WHERE vector_id IS NOT NULL;
