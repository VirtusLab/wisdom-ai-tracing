-- 033_proxy_message_dedup.sql
-- Dedup hook (sessions) vs proxy (llm_calls) usage via the Anthropic message id.
-- Forward-only: old rows have no message id and are never matched.

ALTER TABLE llm_calls ADD COLUMN IF NOT EXISTS anthropic_message_id TEXT;

CREATE TABLE IF NOT EXISTS session_message_ids (
    anthropic_message_id TEXT PRIMARY KEY,
    org_id     UUID NOT NULL REFERENCES orgs(id)     ON DELETE CASCADE,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_session_message_ids_org_msg
    ON session_message_ids (org_id, anthropic_message_id);

CREATE INDEX IF NOT EXISTS idx_llm_calls_message_id
    ON llm_calls (anthropic_message_id) WHERE anthropic_message_id IS NOT NULL;
