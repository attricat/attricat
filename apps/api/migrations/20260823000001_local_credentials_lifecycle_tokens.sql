-- Credential validation and lifecycle state transitions are implemented in Rust.
ALTER TABLE users
    ADD COLUMN email_verified_at TIMESTAMPTZ,
    ADD COLUMN security_version INTEGER NOT NULL DEFAULT 1 CHECK (security_version > 0);

CREATE TABLE local_password_credentials (
    user_id UUID PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    password_hash TEXT NOT NULL,
    credential_version INTEGER NOT NULL DEFAULT 1 CHECK (credential_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE user_lifecycle_action_tokens (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    purpose TEXT NOT NULL CHECK (purpose IN ('email_verification', 'password_setup', 'password_reset')),
    token_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(token_digest) = 32),
    issued_security_version INTEGER NOT NULL CHECK (issued_security_version > 0),
    issued_credential_version INTEGER NOT NULL CHECK (issued_credential_version >= 0),
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (expires_at > created_at),
    CHECK (consumed_at IS NULL OR consumed_at >= created_at),
    CHECK (revoked_at IS NULL OR revoked_at >= created_at)
);
CREATE INDEX user_lifecycle_action_tokens_active_user_purpose_idx
    ON user_lifecycle_action_tokens (user_id, purpose)
    WHERE consumed_at IS NULL AND revoked_at IS NULL;

-- Security-relevant user and credential changes make all outstanding lifecycle
-- links unusable, including links that would otherwise have matching versions.
