-- Per-user Anthropic API keys, used by the transparent LLM proxy
-- (issue softwaremill/tracevault#207, parent #181).
--
-- One row per user. Key stored encrypted at rest (AES-256-GCM via
-- the existing encryption.rs path; same encryption_key env var as
-- org signing keys / SSO client secrets). The plaintext key never
-- leaves the proxy hot path and is never returned through any API.
CREATE TABLE user_anthropic_keys (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    key_encrypted TEXT NOT NULL,
    key_nonce TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
