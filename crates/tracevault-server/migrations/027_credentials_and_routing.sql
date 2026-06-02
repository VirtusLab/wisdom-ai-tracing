-- Step 1 of proxy model-routing: generalize the per-user Anthropic key into a
-- named, multi-credential model, and add a routing-rules table whose per-user
-- default rule (match_model IS NULL) selects which credential the proxy uses.

-- 1. Generalize user_anthropic_keys -> credentials.
ALTER TABLE user_anthropic_keys RENAME TO credentials;

ALTER TABLE credentials ADD COLUMN id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE credentials ADD COLUMN name TEXT NOT NULL DEFAULT 'default';
ALTER TABLE credentials ADD COLUMN protocol TEXT NOT NULL DEFAULT 'anthropic'
    CHECK (protocol IN ('anthropic'));
ALTER TABLE credentials ADD COLUMN base_url TEXT NOT NULL DEFAULT 'https://api.anthropic.com';

ALTER TABLE credentials DROP CONSTRAINT user_anthropic_keys_pkey;
ALTER TABLE credentials ADD PRIMARY KEY (id);
ALTER TABLE credentials ADD CONSTRAINT credentials_user_name_unique UNIQUE (user_id, name);
CREATE INDEX idx_credentials_user_id ON credentials (user_id);

-- 2. Routing rules. Step 1 only uses the default rule (match_model IS NULL).
CREATE TABLE proxy_routing_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    match_model TEXT,
    credential_name TEXT NOT NULL,
    provider_model TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Rules reference a credential by (user_id, name): this keeps routing
    -- decoupled from the key material (rotating a key never touches rules)
    -- while preventing dangling rules that point at a non-existent credential.
    FOREIGN KEY (user_id, credential_name) REFERENCES credentials (user_id, name) ON DELETE CASCADE
);
CREATE UNIQUE INDEX idx_routing_default_per_user
    ON proxy_routing_rules (user_id) WHERE match_model IS NULL;
CREATE UNIQUE INDEX idx_routing_model_per_user
    ON proxy_routing_rules (user_id, match_model) WHERE match_model IS NOT NULL;

-- 3. Backfill: every existing credential gets a default rule pointing at it.
INSERT INTO proxy_routing_rules (user_id, match_model, credential_name)
SELECT user_id, NULL, name FROM credentials;

-- The masking defaults on name/base_url existed only to backfill pre-rename
-- rows. The repo layer always supplies both explicitly, so keep no permanent
-- default that could silently mask an app bug. (id keeps gen_random_uuid() and
-- protocol keeps its sensible 'anthropic' default.)
ALTER TABLE credentials ALTER COLUMN name DROP DEFAULT;
ALTER TABLE credentials ALTER COLUMN base_url DROP DEFAULT;
