-- 034_llm_calls_outcome_check.sql
-- Constrain llm_calls.outcome to the known set, mirroring the CHECK on
-- org_compliance_settings.usage_source (migration 032). Previously outcome was
-- free text, so a typo'd value would persist silently.
--
-- 'client_disconnect' is produced by the response-stream Drop path when a
-- client disconnects mid-stream (see PR #236); it is included here so the two
-- changes compose regardless of merge order.
--
-- Idempotent (guarded by pg_constraint) so re-application is a no-op.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'llm_calls_outcome_check'
    ) THEN
        ALTER TABLE llm_calls
            ADD CONSTRAINT llm_calls_outcome_check
            CHECK (outcome IN ('success', 'upstream_error', 'stream_error', 'client_disconnect'));
    END IF;
END $$;
