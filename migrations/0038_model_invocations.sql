-- Sprint 5.9: Model invocation tracking for Trust Dashboard observability.
-- Records every LLM call: task type, model, tokens, latency, success status.

CREATE TABLE model_invocations (
    id                  INTEGER PRIMARY KEY,
    task                TEXT NOT NULL,          -- "summarization" | "claim_extraction" | "reflection" | "agent"
    model               TEXT NOT NULL,
    input_hash          TEXT NOT NULL,
    input_tokens        INTEGER,
    output_tokens       INTEGER,
    latency_ms          INTEGER,
    success             INTEGER NOT NULL DEFAULT 1,
    error               TEXT,
    created_at          INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX idx_model_invocation_task
    ON model_invocations(task, created_at);
