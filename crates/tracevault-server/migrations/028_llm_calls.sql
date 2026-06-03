-- Per-call ledger written by the Anthropic proxy. One row per forwarded
-- upstream call. input_tokens is stored VERBATIM from the API usage object
-- (already cache-excluded / fresh), matching sessions.input_tokens.
CREATE TABLE llm_calls (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    credential_id UUID REFERENCES credentials(id) ON DELETE SET NULL,
    auth_session_id UUID REFERENCES auth_sessions(id) ON DELETE SET NULL,
    client_session_id TEXT,
    repo_id UUID REFERENCES repos(id) ON DELETE SET NULL,
    branch TEXT,
    requested_model TEXT,
    provider_model TEXT,
    response_model TEXT,
    input_tokens BIGINT,
    output_tokens BIGINT,
    cache_read_tokens BIGINT,
    cache_write_tokens BIGINT,
    total_tokens BIGINT,
    estimated_cost_usd DOUBLE PRECISION,
    stop_reason TEXT,
    http_status INT NOT NULL,
    outcome TEXT NOT NULL, -- 'success' | 'upstream_error'
    duration_ms BIGINT NOT NULL,
    anthropic_request_id TEXT,
    path TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'proxy',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_llm_calls_org_time ON llm_calls(org_id, created_at DESC);
CREATE INDEX idx_llm_calls_user_time ON llm_calls(user_id, created_at DESC);
CREATE INDEX idx_llm_calls_credential ON llm_calls(credential_id);
CREATE INDEX idx_llm_calls_repo ON llm_calls(repo_id) WHERE repo_id IS NOT NULL;
CREATE UNIQUE INDEX idx_llm_calls_request_id ON llm_calls(anthropic_request_id)
    WHERE anthropic_request_id IS NOT NULL;
