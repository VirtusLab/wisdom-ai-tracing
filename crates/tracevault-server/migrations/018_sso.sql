-- SSO configuration per organization
CREATE TABLE org_sso_configs (
    org_id UUID PRIMARY KEY REFERENCES orgs(id) ON DELETE CASCADE,
    issuer_url TEXT NOT NULL,
    client_id TEXT NOT NULL,
    client_secret_encrypted TEXT NOT NULL,
    client_secret_nonce TEXT NOT NULL,
    allowed_domains TEXT[] NOT NULL,
    enforce BOOLEAN NOT NULL DEFAULT true,
    auto_provision BOOLEAN NOT NULL DEFAULT true,
    default_role TEXT NOT NULL DEFAULT 'developer',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Link users to external IdP identities
CREATE TABLE user_sso_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    org_id UUID NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    issuer TEXT NOT NULL,
    subject TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(org_id, subject),
    UNIQUE(user_id, org_id)
);

-- CSRF state for OIDC authorization flow
CREATE TABLE sso_auth_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES orgs(id),
    state TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_sso_auth_requests_state ON sso_auth_requests(state);

-- Allow SSO-provisioned users to have no password
ALTER TABLE users ALTER COLUMN password_hash DROP NOT NULL;
