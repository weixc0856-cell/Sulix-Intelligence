-- Sprint 6.2B: Add artifact_id columns for ArtifactRegistry-backed storage.
--
-- These columns reference the `artifacts` table (migration 0047) and replace
-- inline TEXT storage of large AI-generated content. During the dual-write
-- migration window, both the old TEXT column and the new artifact_id column
-- are populated. The TEXT columns will be dropped in a future sprint.

-- Reflection results: add artifact_id (migrate from result TEXT + artifact_key)
ALTER TABLE reflections ADD COLUMN artifact_id INTEGER REFERENCES artifacts(id);

-- Decision memos: add artifact_id (migrate from memo_json TEXT)
ALTER TABLE decision_records ADD COLUMN memo_artifact_id INTEGER REFERENCES artifacts(id);

-- Claim analysis: add analysis_artifact_id for reasoning + falsification
ALTER TABLE claims ADD COLUMN analysis_artifact_id INTEGER REFERENCES artifacts(id);
