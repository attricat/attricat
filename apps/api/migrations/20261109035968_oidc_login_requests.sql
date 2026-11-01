-- A sign-in in progress at an identity provider. Only digests are stored: the
-- browser keeps the secrets in a cookie, and the callback completes a request
-- once, from the browser that started it.
CREATE TABLE oidc_login_requests (
    state_digest BYTEA PRIMARY KEY CHECK (octet_length(state_digest) = 32),
    binding_digest BYTEA NOT NULL CHECK (octet_length(binding_digest) = 32),
    workspace_id UUID NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    issuer TEXT NOT NULL,
    intent TEXT NOT NULL CHECK (intent IN ('sign_in', 'invitation', 'link')),
    invitation_digest BYTEA CHECK (octet_length(invitation_digest) = 32),
    link_session_digest BYTEA CHECK (octet_length(link_session_digest) = 32),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CHECK ((intent = 'invitation') = (invitation_digest IS NOT NULL)),
    CHECK ((intent = 'link') = (link_session_digest IS NOT NULL))
);
CREATE INDEX oidc_login_requests_workspace_expiry_idx
    ON oidc_login_requests (workspace_id, expires_at);
CREATE INDEX oidc_login_requests_expires_at_idx ON oidc_login_requests (expires_at);
