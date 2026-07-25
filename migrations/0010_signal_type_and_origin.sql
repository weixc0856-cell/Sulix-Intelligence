-- Add signal_type to intelligence_signals and origin to TodaySignal concept.
--
-- signal_type distinguishes entity signals from future signal detection modes:
--   'entity' — entity-anchored signal (V1.5 default)
--   'market' — market-level signal (future)
--   'technology' — technology trend signal (future)
--   'risk' — risk signal (future)
--
-- origin on TodaySignal traces which engine generated a signal,
-- enabling Decision Loop to evaluate prediction accuracy per source.

ALTER TABLE intelligence_signals ADD COLUMN signal_type TEXT NOT NULL DEFAULT 'entity';
