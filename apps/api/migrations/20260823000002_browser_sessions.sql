-- Browser sessions retain only digests of opaque delivery secrets. The API role
-- reaches them through these narrowly scoped functions, never table access.
CREATE TABLE browser_sessions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    session_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(session_digest) = 32),
    csrf_digest BYTEA NOT NULL CHECK (octet_length(csrf_digest) = 32),
    issued_security_version INTEGER NOT NULL CHECK (issued_security_version > 0),
    issued_credential_version INTEGER NOT NULL CHECK (issued_credential_version > 0),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    rotated_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (expires_at > created_at)
);
CREATE INDEX browser_sessions_active_user_workspace_idx
    ON browser_sessions (user_id, workspace_id)
    WHERE revoked_at IS NULL;

CREATE TABLE browser_login_rate_limits (
    key_digest BYTEA PRIMARY KEY CHECK (octet_length(key_digest) = 32),
    window_started_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    failures INTEGER NOT NULL DEFAULT 0 CHECK (failures >= 0)
);
