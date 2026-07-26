-- Sprint 5.10 Phase 0: D1 Quota Recovery — Step 1: Query Optimization
-- Only indexes, no data mutations. Safe to apply independently.

CREATE INDEX IF NOT EXISTS idx_articles_url
ON articles(url);

CREATE INDEX IF NOT EXISTS idx_signal_instances_thread_ts
ON intelligence_signals(signal_thread_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_outbox_status_created
ON object_outbox(status, created_at);
