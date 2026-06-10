-- Per-org analytics usage source: which source(s) feed token/cost metrics.
-- 'both' preserves the existing hook+proxy fold; 'hook'/'proxy' select one,
-- avoiding double-counting when a user runs Claude Code through the proxy
-- while the hook is also installed.
DO $$
BEGIN
    ALTER TABLE org_compliance_settings
        ADD COLUMN usage_source TEXT NOT NULL DEFAULT 'both';
EXCEPTION WHEN duplicate_column THEN
    NULL;
END
$$;

ALTER TABLE org_compliance_settings
    DROP CONSTRAINT IF EXISTS org_compliance_settings_usage_source_check;
ALTER TABLE org_compliance_settings
    ADD CONSTRAINT org_compliance_settings_usage_source_check
    CHECK (usage_source IN ('hook','proxy','both'));
