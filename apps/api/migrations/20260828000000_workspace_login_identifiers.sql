-- Workspace identifiers are normalized once at provisioning and never change. They
-- are routing hints, not DNS names: no ownership or resolution is attempted.
-- The fresh schema contains only the bootstrap workspace at this point. New
-- workspaces are provisioned by Rust and must supply their identifier.
ALTER TABLE workspaces ADD COLUMN login_identifier TEXT NOT NULL DEFAULT 'default.local';
ALTER TABLE workspaces ALTER COLUMN login_identifier DROP DEFAULT;
ALTER TABLE workspaces ADD CONSTRAINT workspaces_login_identifier_key UNIQUE (login_identifier);
ALTER TABLE workspaces ADD CONSTRAINT workspaces_login_identifier_format_check CHECK (
    login_identifier = lower(trim(login_identifier))
    AND char_length(login_identifier) BETWEEN 3 AND 253
    AND login_identifier ~ '^([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}$'
);

CREATE TABLE workspace_discovery_rate_limits (
    key_digest BYTEA PRIMARY KEY CHECK (octet_length(key_digest) = 32),
    window_started_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0)
);
