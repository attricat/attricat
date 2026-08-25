-- Opaque personal API tokens are scoped to one workspace. Their plaintext secret
-- never enters a table: only a SHA-256 digest is retained.
CREATE TABLE personal_api_tokens (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    label TEXT NOT NULL CHECK (label = btrim(label) AND char_length(label) BETWEEN 1 AND 120),
    token_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(token_digest) = 32),
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (expires_at IS NULL OR expires_at > created_at),
    CHECK (revoked_at IS NULL OR revoked_at >= created_at),
    CHECK (last_used_at IS NULL OR last_used_at >= created_at)
);
CREATE INDEX personal_api_tokens_user_workspace_idx ON personal_api_tokens (user_id, workspace_id);

CREATE TABLE personal_api_token_permissions (
    token_id UUID NOT NULL REFERENCES personal_api_tokens (id) ON DELETE CASCADE,
    permission_code TEXT NOT NULL REFERENCES permissions (code) ON DELETE RESTRICT,
    PRIMARY KEY (token_id, permission_code)
);
