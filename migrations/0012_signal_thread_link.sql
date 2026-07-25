-- Link intelligence_signals (instances) to signal_threads.
--
-- Existing rows get NULL signal_thread_id and will be linked
-- as the thread+instance pipeline processes new detections.

ALTER TABLE intelligence_signals ADD COLUMN signal_thread_id INTEGER REFERENCES signal_threads(id);
CREATE INDEX IF NOT EXISTS idx_intel_signals_thread ON intelligence_signals(signal_thread_id);
