-- A workspace's OpenID Connect provider. It starts unverified and off: someone
-- must complete a test sign-in before it can be enabled, and it must be enabled
-- before password sign-in can be restricted, so a bad configuration cannot lock
-- a workspace out. The client secret is stored only as ciphertext bound to the
-- row. A workspace has at most one provider.
CREATE TABLE workspace_identity_providers (
    workspace_id UUID PRIMARY KEY REFERENCES workspaces (id) ON DELETE CASCADE,
    issuer TEXT NOT NULL CHECK (char_length(issuer) BETWEEN 1 AND 2048),
    client_id TEXT NOT NULL CHECK (char_length(client_id) BETWEEN 1 AND 512),
    client_secret_ciphertext BYTEA NOT NULL,
    display_name TEXT NOT NULL CHECK (char_length(display_name) BETWEEN 1 AND 64),
    verified_at TIMESTAMPTZ,
    enabled BOOLEAN NOT NULL DEFAULT false,
    password_sign_in TEXT NOT NULL DEFAULT 'all' CHECK (password_sign_in IN ('all', 'owners')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (NOT enabled OR verified_at IS NOT NULL),
    CHECK (password_sign_in = 'all' OR enabled)
);
