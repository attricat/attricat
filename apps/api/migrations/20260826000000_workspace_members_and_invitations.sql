-- Workspace administration is exposed through narrow database entry points.  In
-- particular, the API role cannot read the global users table or invitation
-- token digests directly.
CREATE TABLE workspace_invitations (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    invitee_email TEXT NOT NULL CHECK (invitee_email = lower(btrim(invitee_email)) AND position('@' IN invitee_email) > 1),
    inviter_user_id UUID NOT NULL REFERENCES users (id),
    role_id UUID NOT NULL REFERENCES roles (id) ON DELETE RESTRICT,
    scope_type TEXT NOT NULL CHECK (scope_type IN ('workspace', 'blueprint_family', 'entity', 'context_subtree')),
    scope_target_id UUID NOT NULL,
    token_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(token_digest) = 32),
    expires_at TIMESTAMPTZ NOT NULL,
    accepted_at TIMESTAMPTZ,
    accepted_by_user_id UUID REFERENCES users (id),
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CHECK (expires_at > created_at),
    CHECK (accepted_by_user_id IS NULL OR accepted_at IS NOT NULL)
);
CREATE INDEX workspace_invitations_workspace_active_idx
    ON workspace_invitations (workspace_id, created_at DESC)
    WHERE accepted_at IS NULL AND revoked_at IS NULL;
