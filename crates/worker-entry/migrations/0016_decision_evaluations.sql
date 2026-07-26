-- Decision Evaluations — Judgment Layer for the Decision Loop.
--
-- Evaluation is separate from Outcome Observation (fact layer).
-- It answers: "was our hypothesis confirmed or contradicted by reality?"
--
-- This enables Decision Accuracy Memory:
--   Decision → Outcome → Evaluation → Accuracy Score → Better Intelligence

CREATE TABLE IF NOT EXISTS decision_evaluations (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    decision_id     INTEGER NOT NULL,

    evaluation      TEXT NOT NULL CHECK (
                        evaluation IN (
                            'confirmed',
                            'partially_confirmed',
                            'contradicted',
                            'inconclusive'
                        )
                    ),

    confidence      REAL CHECK (confidence >= 0 AND confidence <= 1),
    reasoning       TEXT,
    evaluator       TEXT NOT NULL DEFAULT 'manual'
                        CHECK (evaluator IN ('manual', 'ai')),

    evaluated_at    INTEGER NOT NULL,
    created_at      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_evaluations_decision ON decision_evaluations(decision_id);
CREATE INDEX IF NOT EXISTS idx_evaluations_created ON decision_evaluations(created_at);
