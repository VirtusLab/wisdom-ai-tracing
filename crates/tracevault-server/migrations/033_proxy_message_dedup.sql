-- 033_proxy_message_dedup.sql
-- Dedup hook (sessions) vs proxy (llm_calls) usage via the Anthropic message id.
-- Forward-only: old rows have no message id and are never matched.

ALTER TABLE llm_calls ADD COLUMN IF NOT EXISTS anthropic_message_id TEXT;

-- One row per assistant message the hook has accounted for, scoped by org.
-- The composite primary key (org_id, anthropic_message_id) is org-scoped on
-- purpose: a message id is globally unique per Anthropic call, but scoping by
-- org keeps one org's ingestion from "claiming" an id another org could see,
-- and it directly backs the dedup probe (sm.org_id = c.org_id AND
-- sm.anthropic_message_id = c.anthropic_message_id) — so no extra index is
-- needed.
CREATE TABLE IF NOT EXISTS session_message_ids (
    org_id     UUID NOT NULL REFERENCES orgs(id)     ON DELETE CASCADE,
    anthropic_message_id TEXT NOT NULL,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (org_id, anthropic_message_id)
);
