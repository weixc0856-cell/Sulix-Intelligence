-- Sprint 6.1: Upgrade model_invocations for cost tracking + provider routing.
ALTER TABLE model_invocations ADD COLUMN estimated_cost REAL;
ALTER TABLE model_invocations ADD COLUMN provider TEXT;
ALTER TABLE model_invocations ADD COLUMN error_type TEXT;
