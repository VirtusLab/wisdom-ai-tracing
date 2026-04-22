-- Per-policy execution history. Lets users see when policies ran, how they
-- resolved, and which session/commit triggered them. Separate from audit_log
-- because this is high-volume operational data with a natural retention
-- boundary, not a security-audit record.
CREATE TABLE policy_evaluations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    repo_id UUID NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
    -- Nullable so the row survives rule deletion; policy_name carries the
    -- snapshot name for display in historical views.
    policy_id UUID REFERENCES policies(id) ON DELETE SET NULL,
    policy_name TEXT NOT NULL,
    -- String rather than UUID FK: the CLI's /check request carries session
    -- identifiers that are Claude Code session folder names, not rows in
    -- our sessions table.
    session_id TEXT,
    commit_sha TEXT,
    result TEXT NOT NULL, -- 'pass' | 'fail' | 'warn' | 'skip'
    action TEXT NOT NULL, -- 'block_push' | 'warn'
    details TEXT NOT NULL DEFAULT '',
    -- 'cli_check' when driven from pre-push hook, 'ci_verify' when driven
    -- by the post-merge CI verify endpoint.
    source TEXT NOT NULL,
    actor_id UUID REFERENCES users(id) ON DELETE SET NULL,
    evaluated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Primary access pattern: recent evaluations for a repo.
CREATE INDEX idx_policy_eval_repo_time ON policy_evaluations(repo_id, evaluated_at DESC);
-- Per-rule analytics ("how often does X fail?").
CREATE INDEX idx_policy_eval_policy ON policy_evaluations(policy_id);
-- Drill-down from a specific commit.
CREATE INDEX idx_policy_eval_commit ON policy_evaluations(repo_id, commit_sha)
    WHERE commit_sha IS NOT NULL;
